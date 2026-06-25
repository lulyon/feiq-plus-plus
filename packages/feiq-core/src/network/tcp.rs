//! Async TCP for file transfer (IPMSG GETFILEDATA / GETDIRFILES).
//! Chunk size: 64KB for feiq++ <-> feiq++, compatible with legacy 2048-byte clients.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Chunk size for file transfer (64KB for modern networks)
pub const FILE_CHUNK_SIZE: usize = 65536;

/// TCP file transfer server (waits for incoming file requests)
pub struct FileServer {
    listener: TcpListener,
}

impl FileServer {
    /// Start TCP server on the given port
    pub async fn bind(port: u16) -> anyhow::Result<Self> {
        let addr = format!("0.0.0.0:{port}");
        let listener = TcpListener::bind(&addr).await?;
        tracing::info!("TCP file server listening on port {port}");
        Ok(Self { listener })
    }

    /// Accept an incoming file transfer connection
    pub async fn accept(&self) -> anyhow::Result<(TcpStream, String)> {
        let (stream, addr) = self.listener.accept().await?;
        Ok((stream, addr.ip().to_string()))
    }

    /// Get the local address
    pub fn local_addr(&self) -> anyhow::Result<std::net::SocketAddr> {
        Ok(self.listener.local_addr()?)
    }
}

/// TCP client for requesting/transferring files
pub struct FileTransfer {
    stream: TcpStream,
}

impl FileTransfer {
    /// Connect to a remote IPMSG peer for file transfer
    pub async fn connect(ip: &str, port: u16) -> anyhow::Result<Self> {
        let addr = format!("{ip}:{port}");
        let stream = TcpStream::connect(&addr).await?;
        stream.set_nodelay(true)?;
        Ok(Self { stream })
    }

    /// Create from an already-accepted stream
    pub fn from_stream(stream: TcpStream) -> Self {
        let _ = stream.set_nodelay(true);
        Self { stream }
    }

    /// Send raw data
    pub async fn send(&mut self, data: &[u8]) -> anyhow::Result<usize> {
        self.stream.write_all(data).await?;
        Ok(data.len())
    }

    /// Send a file from disk with progress callback.
    /// Returns total bytes sent.
    pub async fn send_file<F>(
        &mut self,
        file_path: &str,
        offset: i64,
        progress_cb: F,
    ) -> anyhow::Result<i64>
    where
        F: Fn(i64, i64),
    {
        let mut file = tokio::fs::File::open(file_path).await?;
        let total = file.metadata().await?.len() as i64;

        if offset > 0 {
            use tokio::io::AsyncSeekExt;
            file.seek(std::io::SeekFrom::Start(offset as u64)).await?;
        }

        let _remaining = total - offset;
        let mut sent: i64 = offset;
        let mut buf = vec![0u8; FILE_CHUNK_SIZE];

        while sent < total {
            let to_read = std::cmp::min(FILE_CHUNK_SIZE, (total - sent) as usize);
            let n = file.read(&mut buf[..to_read]).await?;
            if n == 0 {
                break;
            }
            self.stream.write_all(&buf[..n]).await?;
            sent += n as i64;
            progress_cb(sent, total);
        }

        Ok(sent)
    }

    /// Receive file data and write to disk with progress callback
    pub async fn recv_file<F>(
        &mut self,
        file_path: &str,
        total_size: i64,
        progress_cb: F,
    ) -> anyhow::Result<i64>
    where
        F: Fn(i64, i64),
    {
        let mut file = tokio::fs::File::create(file_path).await?;
        let mut received: i64 = 0;
        let mut buf = vec![0u8; FILE_CHUNK_SIZE];

        while received < total_size {
            let to_read = std::cmp::min(FILE_CHUNK_SIZE, (total_size - received) as usize);
            let n = self.stream.read(&mut buf[..to_read]).await?;
            if n == 0 {
                break; // EOF
            }
            file.write_all(&buf[..n]).await?;
            received += n as i64;
            progress_cb(received, total_size);
        }

        file.flush().await?;
        Ok(received)
    }

    /// Receive raw bytes with timeout
    pub async fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let n = self.stream.read(buf).await?;
        Ok(n)
    }
}
