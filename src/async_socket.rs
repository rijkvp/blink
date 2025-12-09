use anyhow::{Context, Result};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{UnixListener, UnixStream},
};

async fn create_socket_listener(path: &Path, set_permissions: bool) -> Result<UnixListener> {
    if let Some(run_dir) = path.parent() {
        fs::create_dir_all(run_dir)
            .with_context(|| format!("failed to create runtime directory '{run_dir:?}'"))?;
    }
    if path.exists() {
        log::warn!("Removing exsisting socket '{}'", path.display());
        fs::remove_file(path).with_context(|| "failed to remove existing socket")?;
    }
    let listener = tokio::net::UnixListener::bind(path)
        .with_context(|| format!("failed to bind socket at '{path:?}'"))?;
    if set_permissions {
        // set Unix permissions such that all users can write to the socket
        fs::set_permissions(path, fs::Permissions::from_mode(0o722)).unwrap();
    }
    log::info!("Created at socket at '{}'", path.display());
    Ok(listener)
}

async fn create_socket_stream(path: PathBuf) -> Result<UnixStream> {
    let stream = UnixStream::connect(&path)
        .await
        .with_context(|| format!("failed to connect to socket at '{path:?}'"))?;
    Ok(stream)
}

pub struct SocketServer {
    listener: UnixListener,
    path: PathBuf,
}

impl SocketServer {
    pub async fn create(path: PathBuf, set_permissions: bool) -> Result<Self> {
        let listener = create_socket_listener(&path, set_permissions).await?;
        Ok(Self { listener, path })
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub async fn accept_client(&mut self) -> Result<SocketStream> {
        let (stream, _) = self.listener.accept().await?;
        Ok(SocketStream { stream })
    }
}

impl Drop for SocketServer {
    fn drop(&mut self) {
        if self.path.exists() {
            if let Err(e) = fs::remove_file(&self.path) {
                log::warn!(
                    "Failed to remove socket at '{}': {}",
                    self.path.display(),
                    e
                );
            } else {
                log::info!("Removed socket at '{}'", self.path.display());
            }
        }
    }
}

pub struct SocketStream {
    stream: UnixStream,
}

impl SocketStream {
    pub async fn connect(path: PathBuf) -> Result<Self> {
        Ok(Self {
            stream: create_socket_stream(path).await?,
        })
    }

    pub async fn send<T: for<'a> serde::Serialize>(&mut self, msg: T) -> Result<()> {
        let bytes = rmp_serde::to_vec(&msg).with_context(|| "failed to serialize message")?;
        self.stream
            .write_u32(bytes.len() as u32)
            .await
            .context("failed to write message length")?;
        self.stream
            .write_all(&bytes)
            .await
            .context("failed to write message")?;
        self.stream
            .flush()
            .await
            .context("failed to flush stream")?;
        Ok(())
    }

    pub async fn recv<T: for<'a> serde::Deserialize<'a>>(&mut self) -> Result<T> {
        let length = self
            .stream
            .read_u32()
            .await
            .context("failed to read message length")?;
        let mut buf = vec![0; length as usize];
        self.stream
            .read_exact(&mut buf)
            .await
            .context("failed to read message")?;
        let msg: T = rmp_serde::from_slice(&buf).context("failed to deserialize message")?;
        Ok(msg)
    }

    pub async fn send_and_recv<D, S>(&mut self, msg: S) -> Result<D>
    where
        D: for<'a> serde::Deserialize<'a>,
        S: for<'a> serde::Serialize,
    {
        self.send(msg).await?;
        self.recv().await
    }
}
