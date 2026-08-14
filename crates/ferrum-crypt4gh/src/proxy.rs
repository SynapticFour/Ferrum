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
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Lesson 4: bounded channels + cooperative yielding to avoid unbounded buffering.
const PROXY_IN_FLIGHT_IN: usize = 8; // Reader pump -> reencrypt spawn_blocking
const PROXY_IN_FLIGHT_OUT: usize = 4; // spawn_blocking -> async HTTP writer

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

/// Extract `{id}` from `/ga4gh/drs/v1/objects/{id}/stream` (or `/objects/{id}`).
pub fn object_id_from_drs_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let idx = parts.iter().position(|p| *p == "objects")?;
    let id = parts.get(idx + 1)?;
    if id.is_empty() || *id == "stream" || *id == "access" {
        return None;
    }
    Some((*id).to_string())
}

fn json_error_response(
    status: http::StatusCode,
    msg: &'static str,
) -> http::Response<UnsyncBoxBody<Bytes, std::io::Error>> {
    let payload = Bytes::from(format!(r#"{{"error":"{msg}"}}"#));
    let stream = futures_util::stream::once(async move { Ok(Frame::data(payload)) });
    let mut res = http::Response::new(UnsyncBoxBody::new(StreamBody::new(stream)));
    *res.status_mut() = status;
    res.headers_mut().insert(
        http::header::CONTENT_TYPE,
        http::HeaderValue::from_static("application/json"),
    );
    res
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
        let object_id = object_id_from_drs_path(req.uri().path()).unwrap_or_default();

        Box::pin(async move {
            let response = inner.call(req).await?;
            let (parts, body) = response.into_parts();

            // No requester pubkey: pass through (including errors).
            if pubkey_b64.is_none() {
                let stream = body_to_stream(body);
                let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
                return Ok(http::Response::from_parts(parts, new_body));
            }

            if !parts.status.is_success() {
                let stream = body_to_stream(body);
                let new_body = UnsyncBoxBody::new(StreamBody::new(stream));
                return Ok(http::Response::from_parts(parts, new_body));
            }

            // Pubkey present: fail closed rather than forwarding ciphertext.
            if object_id.is_empty() {
                return Ok(json_error_response(
                    http::StatusCode::BAD_REQUEST,
                    "could not determine DRS object id for Crypt4GH rewrap",
                ));
            }

            let Some(claims) = claims else {
                return Ok(json_error_response(
                    http::StatusCode::FORBIDDEN,
                    "authentication required for Crypt4GH rewrap",
                ));
            };
            let visas: Vec<VisaObject> = match &claims {
                AuthClaims::Passport { visas, .. } => visas.clone(),
                AuthClaims::Jwt { .. } => {
                    return Ok(json_error_response(
                        http::StatusCode::FORBIDDEN,
                        "Passport visas required for Crypt4GH rewrap",
                    ));
                }
            };
            let subject_id = match &claims {
                AuthClaims::Passport { claims: c, .. } => c.sub.as_deref().unwrap_or(""),
                _ => "",
            };

            if !config.policy_engine.check(&object_id, &visas, subject_id) {
                return Ok(json_error_response(
                    http::StatusCode::FORBIDDEN,
                    "Crypt4GH rewrap denied by policy",
                ));
            }

            let pubkey_b64 = pubkey_b64.unwrap();
            let pubkey = match base64::engine::general_purpose::STANDARD.decode(pubkey_b64.trim()) {
                Ok(p) => p,
                Err(_) => {
                    return Ok(json_error_response(
                        http::StatusCode::BAD_REQUEST,
                        "invalid X-Crypt4GH-Public-Key",
                    ));
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
                    return Ok(json_error_response(
                        http::StatusCode::BAD_GATEWAY,
                        "Crypt4GH master key unavailable",
                    ));
                }
            };

            let (tx_in, rx_in) = tokio::sync::mpsc::channel::<Bytes>(PROXY_IN_FLIGHT_IN);
            let (tx_out, rx_out) = tokio::sync::mpsc::channel::<Bytes>(PROXY_IN_FLIGHT_OUT);
            let mut reader = ChannelReader::new(rx_in);
            let mut writer = ChannelWriter::new(tx_out);
            let keys = master_keys.clone();
            let recipients = recipient_keys.clone();

            tokio::spawn(async move {
                let mut stream = BodyStream::new(body);
                while let Some(Ok(frame)) = stream.next().await {
                    if let Ok(data) = frame.into_data() {
                        if tx_in.send(data).await.is_err() {
                            break;
                        }
                    }
                }
            });

            let join = tokio::task::spawn_blocking(move || {
                crypt4gh::reencrypt(&keys, &recipients, &mut reader, &mut writer, true)
            });
            tokio::spawn(async move {
                match join.await {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => tracing::error!(error = %e, "Crypt4GH proxy reencrypt failed"),
                    Err(e) => tracing::error!(error = %e, "Crypt4GH proxy reencrypt join failed"),
                }
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
    let (tx, rx) = tokio::sync::mpsc::channel(32);
    tokio::spawn(async move {
        let mut stream = BodyStream::new(body);
        while let Some(Ok(frame)) = stream.next().await {
            if let Ok(data) = frame.into_data() {
                if tx.send(Ok(Frame::data(data))).await.is_err() {
                    break;
                }
            }
        }
    });
    BodyStreamAdapter { rx }
}

struct BodyStreamAdapter {
    rx: tokio::sync::mpsc::Receiver<Result<Frame<Bytes>, std::io::Error>>,
}

impl Stream for BodyStreamAdapter {
    type Item = Result<Frame<Bytes>, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().rx.poll_recv(cx)
    }
}

struct ReencryptStream {
    rx: tokio::sync::mpsc::Receiver<Bytes>,
}

impl Stream for ReencryptStream {
    type Item = Result<Frame<Bytes>, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.get_mut().rx.poll_recv(cx) {
            Poll::Ready(Some(chunk)) => Poll::Ready(Some(Ok(Frame::data(chunk)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::object_id_from_drs_path;

    #[test]
    fn object_id_from_stream_path() {
        assert_eq!(
            object_id_from_drs_path("/ga4gh/drs/v1/objects/abc123/stream").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            object_id_from_drs_path("/ga4gh/drs/v1/objects/abc123").as_deref(),
            Some("abc123")
        );
        assert_eq!(
            object_id_from_drs_path("/ga4gh/drs/v1/objects/stream"),
            None
        );
        assert_eq!(object_id_from_drs_path("/health"), None);
    }
}
