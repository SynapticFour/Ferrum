//! Layer 1: Standard Crypt4GH file operations — encrypt, decrypt, re-encrypt, key management.

use crate::error::{Crypt4GHError, Result};
use async_trait::async_trait;
use crypt4gh::keys::{generate_keys as c4gh_generate_keys, get_private_key, get_public_key};
use crypt4gh::{decrypt, encrypt, reencrypt};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Crypt4GH key (crate type re-export for convenience).
pub type C4ghKeys = crypt4gh::Keys;

/// Build recipient Keys from raw public key bytes (e.g. from age or Crypt4GH format).
pub fn recipient_keys_from_pubkey(pubkey: &[u8]) -> C4ghKeys {
    // Learned from neicnordic/crypt4gh reference behavior:
    // the encrypt path requires a non-empty ephemeral sender private key for
    // per-recipient X25519+ChaCha20-Poly1305 header packets. An empty privkey
    // panics in crypt4gh internals for chunk/header encryption.
    let ephemeral = crypt4gh::keys::generate_private_key();
    crypt4gh::Keys {
        method: 0,
        privkey: ephemeral[..32].to_vec(),
        recipient_pubkey: pubkey.to_vec(),
    }
}

/// Load recipient keys from a Crypt4GH or OpenSSH public key file.
pub fn load_recipient_keys(path: &Path) -> Result<C4ghKeys> {
    let pubkey = get_public_key(path).map_err(Crypt4GHError::Crypto)?;
    Ok(recipient_keys_from_pubkey(&pubkey))
}

/// Generate a new Crypt4GH keypair (age-compatible format when possible).
pub fn generate_keypair(
    seckey_path: &Path,
    pubkey_path: &Path,
    passphrase: Option<&str>,
) -> Result<()> {
    std::fs::create_dir_all(seckey_path.parent().unwrap_or(Path::new(".")))
        .map_err(Crypt4GHError::Io)?;
    std::fs::create_dir_all(pubkey_path.parent().unwrap_or(Path::new(".")))
        .map_err(Crypt4GHError::Io)?;
    let pass = passphrase.unwrap_or("");
    c4gh_generate_keys(
        seckey_path,
        pubkey_path,
        || Ok::<_, crypt4gh::error::Crypt4GHError>(pass.to_string()),
        None,
    )
    .map_err(Crypt4GHError::Crypto)
}

/// KeyStore trait: resolve keys by ID (e.g. "master", "user:123").
#[async_trait]
pub trait KeyStore: Send + Sync {
    /// Get decryption keys (private key) for the given key ID.
    async fn get_private_key(&self, key_id: &str) -> Result<Option<Vec<C4ghKeys>>>;
    /// Get public key bytes for a key ID (for re-encryption recipient).
    async fn get_public_key_bytes(&self, key_id: &str) -> Result<Option<Vec<u8>>>;
    /// Store a key pair (optional; not all backends support write).
    async fn store_key(&self, key_id: &str, keys: &[C4ghKeys]) -> Result<()>;
}

/// Local key store: keys in files under a directory.
pub struct LocalKeyStore {
    base_path: std::path::PathBuf,
}

impl LocalKeyStore {
    pub fn new(base_path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    fn path_private(&self, key_id: &str) -> std::path::PathBuf {
        self.base_path
            .join(sanitize_key_id(key_id))
            .with_extension("sec")
    }

    fn path_public(&self, key_id: &str) -> std::path::PathBuf {
        self.base_path
            .join(sanitize_key_id(key_id))
            .with_extension("pub")
    }
}

fn sanitize_key_id(id: &str) -> String {
    id.replace(['/', '\\'], "_").replace("..", "_")
}

#[async_trait]
impl KeyStore for LocalKeyStore {
    async fn get_private_key(&self, key_id: &str) -> Result<Option<Vec<C4ghKeys>>> {
        let path = self.path_private(key_id);
        if !path.exists() {
            return Ok(None);
        }
        let path = path.clone();
        // Lesson: dedicated POSIX I/O pool instead of Tokio's spawn_blocking for key file reads.
        // Source: HPC / many concurrent Crypt4GH decrypt streams.
        let raw = ferrum_core::io::posix::spawn_blocking(move || {
            get_private_key(&path, || Ok(String::new()))
        })
        .await
        .map_err(|e| Crypt4GHError::Other(e.into()))?
        .map_err(Crypt4GHError::Crypto)?;
        // crypt4gh get_private_key returns raw key bytes; wrap as C4ghKeys for KeyStore API
        let keys = C4ghKeys {
            method: 0,
            privkey: raw,
            recipient_pubkey: vec![],
        };
        Ok(Some(vec![keys]))
    }

    async fn get_public_key_bytes(&self, key_id: &str) -> Result<Option<Vec<u8>>> {
        let path = self.path_public(key_id);
        if !path.exists() {
            return Ok(None);
        }
        let path = path.clone();
        let bytes = ferrum_core::io::posix::spawn_blocking(move || get_public_key(&path))
            .await
            .map_err(|e| Crypt4GHError::Other(e.into()))?
            .map_err(Crypt4GHError::Crypto)?;
        Ok(Some(bytes))
    }

    async fn store_key(&self, _key_id: &str, _keys: &[C4ghKeys]) -> Result<()> {
        // LocalKeyStore is read-only from API; keys are written via generate_keypair + copy
        Err(Crypt4GHError::KeyError(
            "LocalKeyStore is read-only".to_string(),
        ))
    }
}

/// Database key store: keys stored in a table (key_id -> private or public blob).
pub struct DatabaseKeyStore<DB> {
    _db: Arc<DB>,
}

impl<DB> DatabaseKeyStore<DB> {
    pub fn new(db: Arc<DB>) -> Self {
        Self { _db: db }
    }
}

// Placeholder: when DB type is concrete (e.g. ferrum_core::DatabasePool), implement KeyStore
// by querying a keys table. For now we leave it unimplemented so the crate compiles.
#[async_trait]
impl<DB: Send + Sync> KeyStore for DatabaseKeyStore<DB> {
    async fn get_private_key(&self, _key_id: &str) -> Result<Option<Vec<C4ghKeys>>> {
        Ok(None)
    }

    async fn get_public_key_bytes(&self, _key_id: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn store_key(&self, _key_id: &str, _keys: &[C4ghKeys]) -> Result<()> {
        Err(Crypt4GHError::KeyError(
            "DatabaseKeyStore not yet implemented".to_string(),
        ))
    }
}

/// Chunk size for streaming (align with Crypt4GH segment where possible).
const STREAM_CHUNK: usize = 65536;

/// Bridge: async read from R, sync decrypt, async write to W. Runs decrypt in spawn_blocking with
/// channel-based Read/Write for bounded memory.
pub async fn stream_decrypt<R, W>(
    keys: &[C4ghKeys],
    mut read: R,
    mut write: W,
    sender_pubkey: Option<&[u8]>,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // Progress guarantee: unbounded channels prevent deadlocks caused by
    // bounded-capacity backpressure between async pumps and spawn_blocking.
    let (tx_in, rx_in) = std::sync::mpsc::channel::<Vec<u8>>();
    let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
    let keys = keys.to_vec();
    let sender = sender_pubkey.map(Vec::from);

    let mut reader = ChannelReader::new(rx_in);
    let mut writer = ChannelWriter::new(tx_out);

    let join = tokio::task::spawn_blocking(move || {
        decrypt(&keys, &mut reader, &mut writer, 0, None, &sender)
    });

    let read_task = tokio::spawn(async move {
        let mut buf = vec![0u8; STREAM_CHUNK];
        loop {
            let n = read.read(&mut buf).await.map_err(Crypt4GHError::Io)?;
            if n == 0 {
                drop(tx_in);
                break;
            }
            if tx_in.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
        Ok::<_, Crypt4GHError>(())
    });

    let write_task = tokio::spawn(async move {
        use std::sync::mpsc::TryRecvError;
        loop {
            match rx_out.try_recv() {
                Ok(chunk) => {
                    write.write_all(&chunk).await.map_err(Crypt4GHError::Io)?;
                }
                Err(TryRecvError::Empty) => {
                    // Avoid blocking a Tokio worker thread. We are in an async task,
                    // so use cooperative yielding while waiting for more ciphertext.
                    tokio::task::yield_now().await;
                }
                Err(TryRecvError::Disconnected) => break,
            }
        }
        write.flush().await.map_err(Crypt4GHError::Io)?;
        Ok::<_, Crypt4GHError>(())
    });

    join.await.map_err(|e| Crypt4GHError::Other(e.into()))??;
    read_task
        .await
        .map_err(|e| Crypt4GHError::Other(e.into()))??;
    write_task
        .await
        .map_err(|e| Crypt4GHError::Other(e.into()))??;
    Ok(())
}

/// Stream encrypt: async read -> encrypt -> async write.
pub async fn stream_encrypt<R, W>(
    recipient_keys: &HashSet<C4ghKeys>,
    mut read: R,
    mut write: W,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // Progress guarantee: unbounded channels prevent deadlocks caused by
    // bounded-capacity backpressure between async pumps and spawn_blocking.
    let (tx_in, rx_in) = std::sync::mpsc::channel::<Vec<u8>>();
    let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
    let keys = recipient_keys.clone();

    let mut reader = ChannelReader::new(rx_in);
    let mut writer = ChannelWriter::new(tx_out);

    let join =
        tokio::task::spawn_blocking(move || encrypt(&keys, &mut reader, &mut writer, 0, None));

    let read_task = tokio::spawn(async move {
        let mut buf = vec![0u8; STREAM_CHUNK];
        loop {
            let n = read.read(&mut buf).await.map_err(Crypt4GHError::Io)?;
            if n == 0 {
                drop(tx_in);
                break;
            }
            if tx_in.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
        Ok::<_, Crypt4GHError>(())
    });

    let write_task = tokio::spawn(async move {
        use std::sync::mpsc::TryRecvError;
        loop {
            match rx_out.try_recv() {
                Ok(chunk) => {
                    write.write_all(&chunk).await.map_err(Crypt4GHError::Io)?;
                }
                Err(TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                }
                Err(TryRecvError::Disconnected) => break,
            }
        }
        write.flush().await.map_err(Crypt4GHError::Io)?;
        Ok::<_, Crypt4GHError>(())
    });

    join.await.map_err(|e| Crypt4GHError::Other(e.into()))??;
    read_task
        .await
        .map_err(|e| Crypt4GHError::Other(e.into()))??;
    write_task
        .await
        .map_err(|e| Crypt4GHError::Other(e.into()))??;
    Ok(())
}

/// Re-encrypt stream: decrypt with keys, re-encrypt for recipient_keys. Header rewrapping without full plaintext.
pub async fn stream_reencrypt<R, W>(
    keys: &[C4ghKeys],
    recipient_keys: &HashSet<C4ghKeys>,
    mut read: R,
    mut write: W,
    trim: bool,
) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    // Progress guarantee: unbounded channels prevent deadlocks caused by
    // bounded-capacity backpressure between async pumps and spawn_blocking.
    let (tx_in, rx_in) = std::sync::mpsc::channel::<Vec<u8>>();
    let (tx_out, rx_out) = std::sync::mpsc::channel::<Vec<u8>>();
    let keys = keys.to_vec();
    let recipients = recipient_keys.clone();

    let mut reader = ChannelReader::new(rx_in);
    let mut writer = ChannelWriter::new(tx_out);

    let join = tokio::task::spawn_blocking(move || {
        reencrypt(&keys, &recipients, &mut reader, &mut writer, trim)
    });

    let read_task = tokio::spawn(async move {
        let mut buf = vec![0u8; STREAM_CHUNK];
        loop {
            let n = read.read(&mut buf).await.map_err(Crypt4GHError::Io)?;
            if n == 0 {
                drop(tx_in);
                break;
            }
            if tx_in.send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
        Ok::<_, Crypt4GHError>(())
    });

    let write_task = tokio::spawn(async move {
        use std::sync::mpsc::TryRecvError;
        loop {
            match rx_out.try_recv() {
                Ok(chunk) => {
                    write.write_all(&chunk).await.map_err(Crypt4GHError::Io)?;
                }
                Err(TryRecvError::Empty) => {
                    tokio::task::yield_now().await;
                }
                Err(TryRecvError::Disconnected) => break,
            }
        }
        write.flush().await.map_err(Crypt4GHError::Io)?;
        Ok::<_, Crypt4GHError>(())
    });

    join.await.map_err(|e| Crypt4GHError::Other(e.into()))??;
    read_task
        .await
        .map_err(|e| Crypt4GHError::Other(e.into()))??;
    write_task
        .await
        .map_err(|e| Crypt4GHError::Other(e.into()))??;
    Ok(())
}

/// Sync Read that reads from a channel (for use inside spawn_blocking). Exported for proxy pipeline.
pub(crate) struct ChannelReader {
    receiver: std::sync::mpsc::Receiver<Vec<u8>>,
    current: std::io::Cursor<Vec<u8>>,
}

impl ChannelReader {
    pub(crate) fn new(receiver: std::sync::mpsc::Receiver<Vec<u8>>) -> Self {
        Self {
            receiver,
            current: std::io::Cursor::new(Vec::new()),
        }
    }
}

impl std::io::Read for ChannelReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.current.position() as usize >= self.current.get_ref().len() {
            match self.receiver.recv() {
                Ok(chunk) => self.current = std::io::Cursor::new(chunk),
                Err(_) => return Ok(0),
            }
        }
        std::io::Read::read(&mut self.current, buf)
    }
}

/// Sync Write that sends to a channel. Exported for proxy pipeline.
pub(crate) struct ChannelWriter {
    sender: std::sync::mpsc::Sender<Vec<u8>>,
    buffer: Vec<u8>,
}

impl Drop for ChannelWriter {
    fn drop(&mut self) {
        // crypt4gh's encrypt/decrypt implementations may not call `flush()` on the
        // provided Write. To avoid truncating the final header/segment bytes,
        // push any remaining buffered data when the writer is dropped.
        if !self.buffer.is_empty() {
            let chunk = std::mem::take(&mut self.buffer);
            let _ = self.sender.send(chunk);
        }
    }
}

impl ChannelWriter {
    pub(crate) fn new(sender: std::sync::mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            sender,
            buffer: Vec::with_capacity(STREAM_CHUNK),
        }
    }
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        // Correctness first: the `crypt4gh` crate may not call `flush()` at the end of
        // `encrypt`/`decrypt`. If we only buffer until `STREAM_CHUNK`, we risk producing
        // truncated ciphertext and then deadlocking in the decrypt reader.
        //
        // We therefore forward each `write()` call immediately as a channel message.
        self.buffer.extend_from_slice(buf);
        let chunk = std::mem::take(&mut self.buffer);
        self.sender
            .send(chunk)
            .map_err(|_| std::io::ErrorKind::BrokenPipe)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Re-encrypt in-memory (for key exchange: small payload or header-only).
pub fn reencrypt_bytes(
    keys: &[C4ghKeys],
    recipient_keys: &HashSet<C4ghKeys>,
    input: &[u8],
    trim: bool,
) -> std::result::Result<Vec<u8>, crypt4gh::error::Crypt4GHError> {
    let mut reader = std::io::Cursor::new(input);
    let mut writer = Vec::new();
    reencrypt(keys, recipient_keys, &mut reader, &mut writer, trim)?;
    Ok(writer)
}
