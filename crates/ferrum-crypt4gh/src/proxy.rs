//! Layer 2: Transparent DRS proxy — stream re-encrypt for requester, Tower middleware.

use crate::encryption::{recipient_keys_from_pubkey, ChannelReader, ChannelWriter, KeyStore};
use crate::policy::PolicyEngine;
use base64::Engine;
use bytes::Bytes;
use ferrum_core::auth::{AuthClaims, VisaObject};
use futures_util::stream::{Stream, StreamExt};
use http;
use http_body_util::{combinators::UnsyncBoxBody, BodyStream, StreamBody};
use hyper::body::Frame;
use std::sync::mpsc;
use std::sync::Arc;
use tower::{Layer, Service};

/// Custom header for requester's Crypt4GH public key (base64).
pub const HEADER_CRYPT4GH_PUBLIC_KEY: &str = "x-crypt4gh-public-key";

/// Configuration for the Crypt4GH proxy layer.
pub struct Crypt4GHProxyConfig {
    pub key_store: Arc<dyn KeyStore>,
    pub policy_engine: Arc<PolicyEngine>,
    pub master_key_id: String,
}

/// Tower Layer that wraps a service and applies Crypt4GH re-encryption when the
/// requester has a valid visa and provides X-Crypt4GH-Public-Key.
pub struct Crypt4GHLayer {
    config: Arc<Crypt4GHProxyConfig>,
}

impl Crypt4GHLayer {
    pub fn new(config: Crypt4GHProxyConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl<S> Layer<S> for Crypt4GHLayer {
    type Service = Crypt4GHProxyService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        Crypt4GHProxyService {
            inner,
            config: Arc::clone(&self.config),
        }
    }
}

pub struct Crypt4GHProxyService<S> {
    inner: S,
    config: Arc<Crypt4GHProxyConfig>,
}

impl<S, ReqBody, ResBody> Service<http::Request<ReqBody>> for Crypt4GHProxyService<S>
where
    S: Service<http::Request<ReqBody>, Response = http::Response<ResBody>> + Clone + Send + 'static,
    S::Future: Send,
    ReqBody: Send + 'static,
    ResBody: http_body::Body<Data = Bytes, Error = std::io::Error> + Send + Unpin + 'static,
    ResBody::Data: Send,
{
    type Response = http::Response<UnsyncBoxBody<Bytes, std::io::Error>>;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<ReqBody>) -> Self::Future {
        let config = Arc::clone(&self.config);
        let mut inner = self.inner.clone();
        let claims = req.extensions().get::<AuthClaims>().cloned();
        let pubkey_b64 = req
            .headers()
            .get(HEADER_CRYPT4GH_PUBLIC_KEY)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let object_id = req
            .uri()
            .path()
            .split('/')
            .rfind(|s| !s.is_empty())
            .unwrap_or("")
            .to_string();

        Box::pin(async move {
            let response = inner.call(req).await?;
            let (parts, body) = response.into_parts();

            let should_reencrypt =
                claims.is_some() && pubkey_b64.is_some() && !object_id.is_empty();

            if !should_reencrypt {
                let stream = body_to_stream(body);
                let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
                return Ok(http::Response::from_parts(parts, new_body));
            }

            let claims = claims.unwrap();
            let pubkey_b64 = pubkey_b64.unwrap();
            let visas: Vec<VisaObject> = match &claims {
                AuthClaims::Jwt { .. } => {
                    let stream = body_to_stream(body);
                    let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
                    return Ok(http::Response::from_parts(parts, new_body));
                }
                AuthClaims::Passport { visas, .. } => visas.clone(),
            };
            let subject_id = match &claims {
                AuthClaims::Passport { claims: c, .. } => c.sub.as_deref().unwrap_or(""),
                _ => "",
            };

            if !config.policy_engine.check(&object_id, &visas, subject_id) {
                let stream = body_to_stream(body);
                let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
                return Ok(http::Response::from_parts(parts, new_body));
            }

            let pubkey = match base64::engine::general_purpose::STANDARD.decode(pubkey_b64.trim()) {
                Ok(p) => p,
                Err(_) => {
                    let stream = body_to_stream(body);
                    let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
                    return Ok(http::Response::from_parts(parts, new_body));
                }
            };
            let recipient_keys =
                std::collections::HashSet::from([recipient_keys_from_pubkey(&pubkey)]);

            let master_keys = match config
                .key_store
                .get_private_key(&config.master_key_id)
                .await
            {
                Ok(Some(k)) => k,
                _ => {
                    let stream = body_to_stream(body);
                    let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
                    return Ok(http::Response::from_parts(parts, new_body));
                }
            };

            let (tx_in, rx_in) = mpsc::sync_channel(32);
            let (tx_out, rx_out) = mpsc::sync_channel(32);
            let mut reader = ChannelReader::new(rx_in);
            let mut writer = ChannelWriter::new(tx_out);
            let keys = master_keys.clone();
            let recipients = recipient_keys.clone();

            tokio::spawn(async move {
                let mut stream = BodyStream::new(body);
                while let Some(Ok(frame)) = stream.next().await {
                    if let Ok(data) = frame.into_data() {
                        let _ = tx_in.send(data.to_vec());
                    }
                }
                drop(tx_in);
            });

            tokio::task::spawn_blocking(move || {
                crypt4gh::reencrypt(&keys, &recipients, &mut reader, &mut writer, true)
            });

            let stream = ReencryptStream { rx: rx_out };
            let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
            Ok(http::Response::from_parts(parts, new_body))
        })
    }
}

/// Adapter: turn an HTTP Body into a Stream of Frame by polling in a task and sending to channel.
fn body_to_stream<B>(body: B) -> impl Stream<Item = Result<Frame<Bytes>, std::io::Error>> + Send
where
    B: http_body::Body<Data = Bytes, Error = std::io::Error> + Send + Unpin + 'static,
{
    let (tx, rx) = mpsc::sync_channel(32);
    tokio::spawn(async move {
        let mut stream = BodyStream::new(body);
        while let Some(Ok(frame)) = stream.next().await {
            if let Ok(data) = frame.into_data() {
                let _ = tx.send(Ok(Frame::data(data)));
            }
        }
        drop(tx);
    });
    BodyStreamAdapter { rx }
}

struct BodyStreamAdapter {
    rx: mpsc::Receiver<Result<Frame<Bytes>, std::io::Error>>,
}

impl Stream for BodyStreamAdapter {
    type Item = Result<Frame<Bytes>, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        match self.rx.try_recv() {
            Ok(item) => std::task::Poll::Ready(Some(item)),
            Err(mpsc::TryRecvError::Empty) => std::task::Poll::Pending,
            Err(mpsc::TryRecvError::Disconnected) => std::task::Poll::Ready(None),
        }
    }
}

struct ReencryptStream {
    rx: mpsc::Receiver<Vec<u8>>,
}

impl Stream for ReencryptStream {
    type Item = Result<Frame<Bytes>, std::io::Error>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let rx = &self.rx;
        match rx.try_recv() {
            Ok(chunk) => std::task::Poll::Ready(Some(Ok(Frame::data(Bytes::from(chunk))))),
            Err(mpsc::TryRecvError::Empty) => {
                cx.waker().wake_by_ref();
                std::task::Poll::Pending
            }
            Err(mpsc::TryRecvError::Disconnected) => std::task::Poll::Ready(None),
        }
    }
}
