//! Async TCP for file transfer (IPMSG GETFILEDATA / GETDIRFILES).
//! Chunk size: 64KB for feiq++ <-> feiq++, compatible with legacy 2048-byte clients.

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

use crate::protocol::constants::{IPMSG_FILE_DIR, IPMSG_FILE_REGULAR};
use crate::protocol::types::FileContent;

/// List files in a directory, returning IPMSG-formatted file entries.
/// Recursively lists all files under `dir_path`.
/// File paths are set to the absolute filesystem path for serving.
/// `base_path` is documented for future relative-path computation.
pub fn list_directory(dir_path: &str, _base_path: &str) -> Vec<FileContent> {
    let mut files = Vec::new();

    let dir = match std::fs::read_dir(dir_path) {
        Ok(dir) => dir,
        Err(e) => {
            tracing::warn!("Failed to read directory {}: {}", dir_path, e);
            return files;
        }
    };

    for entry in dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        let path = entry.path();
        let metadata = match std::fs::metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        if filename.starts_with('.') {
            // Skip hidden files
            continue;
        }

        let file_type = if metadata.is_dir() {
            IPMSG_FILE_DIR
        } else if metadata.is_file() {
            IPMSG_FILE_REGULAR
        } else {
            continue; // Skip symlinks, devices, etc.
        };

        let file_content = FileContent {
            file_id: 0, // assigned by caller
            filename,
            path: path.to_string_lossy().to_string(), // full path for serving
            size: metadata.len() as i64,
            modify_time: metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            file_type,
            packet_no: 0,
            local_task_id: None,
        };

        // Recursively list subdirectories
        if metadata.is_dir() {
            let sub_files = list_directory(&path.to_string_lossy(), _base_path);
            files.push(file_content);
            files.extend(sub_files);
        } else {
            files.push(file_content);
        }
    }

    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_dir() -> (String, std::path::PathBuf) {
        let pid = std::process::id();
        let path = std::path::PathBuf::from(format!(
            "/tmp/feiq_test_listdir_{}_{}",
            pid,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        (path.to_string_lossy().to_string(), path)
    }

    #[test]
    fn test_list_directory_returns_correct_types() {
        let (dir_path, path) = create_temp_dir();

        // Create a regular file
        let file_path = path.join("test.txt");
        std::fs::write(&file_path, "hello").unwrap();

        // Create a subdirectory
        let sub_dir = path.join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();

        let files = list_directory(&dir_path, &dir_path);

        assert!(!files.is_empty(), "Should list at least one entry");

        let txt = files.iter().find(|f| f.filename == "test.txt").unwrap();
        assert_eq!(txt.file_type, IPMSG_FILE_REGULAR);
        assert_eq!(txt.size, 5);

        let sub = files.iter().find(|f| f.filename == "subdir").unwrap();
        assert_eq!(sub.file_type, IPMSG_FILE_DIR);

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn test_list_directory_empty_dir() {
        let (dir_path, path) = create_temp_dir();

        let files = list_directory(&dir_path, &dir_path);
        assert!(files.is_empty(), "Empty directory should return no files");

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn test_list_directory_nonexistent_path() {
        let files = list_directory("/tmp/feiq_test_nonexistent_dir_12345", "");
        assert!(files.is_empty(), "Nonexistent path should return empty vec");
    }

    #[test]
    fn test_list_directory_hidden_files_skipped() {
        let (dir_path, path) = create_temp_dir();

        std::fs::write(path.join("visible.txt"), "data").unwrap();
        std::fs::write(path.join(".hidden"), "secret").unwrap();

        let files = list_directory(&dir_path, &dir_path);
        assert!(
            files.iter().any(|f| f.filename == "visible.txt"),
            "visible.txt should be listed"
        );
        assert!(
            !files.iter().any(|f| f.filename == ".hidden"),
            ".hidden should be excluded"
        );

        let _ = std::fs::remove_dir_all(&path);
    }
}

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
    /// Connect to a remote IPMSG peer for file transfer (30s timeout)
    pub async fn connect(ip: &str, port: u16) -> anyhow::Result<Self> {
        let addr = format!("{ip}:{port}");
        let stream = timeout(Duration::from_secs(30), TcpStream::connect(&addr))
            .await
            .map_err(|_| anyhow::anyhow!("TCP connect to {addr} timed out after 30s"))??;
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
    /// Returns total bytes sent. Each read/write has a 300s timeout.
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
            let n = timeout(Duration::from_secs(300), file.read(&mut buf[..to_read]))
                .await
                .map_err(|_| anyhow::anyhow!("send_file: disk read timed out after 300s"))??;
            if n == 0 {
                break;
            }
            timeout(Duration::from_secs(300), self.stream.write_all(&buf[..n]))
                .await
                .map_err(|_| anyhow::anyhow!("send_file: socket write timed out after 300s"))??;
            sent += n as i64;
            progress_cb(sent, total);
        }

        Ok(sent)
    }

    /// Receive file data and write to disk with progress callback.
    /// Rejects files larger than MAX_FILE_SIZE to prevent DoS attacks
    /// where a malicious peer declares a fake huge file size.
    /// Each read/write has a 300s timeout.
    pub async fn recv_file<F>(
        &mut self,
        file_path: &str,
        total_size: i64,
        progress_cb: F,
    ) -> anyhow::Result<i64>
    where
        F: Fn(i64, i64),
    {
        const MAX_FILE_SIZE: i64 = 100 * 1024 * 1024 * 1024; // 100 GB sanity limit

        if total_size > MAX_FILE_SIZE {
            return Err(anyhow::anyhow!(
                "File size {total_size} exceeds maximum {MAX_FILE_SIZE}"
            ));
        }
        if total_size < 0 {
            return Err(anyhow::anyhow!("Invalid negative file size: {total_size}"));
        }

        let mut file = tokio::fs::File::create(file_path).await?;
        let mut received: i64 = 0;
        let mut buf = vec![0u8; FILE_CHUNK_SIZE];

        while received < total_size {
            let to_read = std::cmp::min(FILE_CHUNK_SIZE, (total_size - received) as usize);
            let n = timeout(Duration::from_secs(300), self.stream.read(&mut buf[..to_read]))
                .await
                .map_err(|_| anyhow::anyhow!("recv_file: socket read timed out after 300s"))??;
            if n == 0 {
                break; // EOF
            }
            timeout(Duration::from_secs(300), file.write_all(&buf[..n]))
                .await
                .map_err(|_| anyhow::anyhow!("recv_file: disk write timed out after 300s"))??;
            received += n as i64;
            progress_cb(received, total_size);
        }

        file.flush().await?;

        if received < total_size {
            return Err(anyhow::anyhow!(
                "Incomplete download: received {received} of {total_size} bytes (early EOF)"
            ));
        }

        Ok(received)
    }

    /// Receive raw bytes with timeout
    pub async fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let n = self.stream.read(buf).await?;
        Ok(n)
    }
}
