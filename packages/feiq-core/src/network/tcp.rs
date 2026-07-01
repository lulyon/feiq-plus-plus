//! Async TCP for file transfer (IPMSG GETFILEDATA / GETDIRFILES).
//! Chunk size: 64KB for feiq++ <-> feiq++, compatible with legacy 2048-byte clients.

use socket2::{Domain, Protocol, Socket, Type};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::{timeout, Duration};

use crate::protocol::constants::{IPMSG_FILE_DIR, IPMSG_FILE_REGULAR};
use crate::protocol::types::{FileContent, FolderFileEntry, FolderManifest};
use std::sync::atomic::{AtomicBool, Ordering};

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
        // Use symlink_metadata to detect symlink directories (prevents infinite recursion
        // from symlinks pointing to parent directories).
        let sym_meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if sym_meta.file_type().is_symlink() {
            // Skip symlink directories to prevent infinite recursion.
            // Symlink files are skipped to prevent traversal attacks.
            continue;
        }
        let metadata = sym_meta;

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
            continue; // Skip devices, etc.
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

    #[test]
    fn test_build_folder_manifest_basic() {
        let (dir_path, path) = create_temp_dir();

        std::fs::write(path.join("a.txt"), "hello").unwrap();
        std::fs::write(path.join("b.txt"), "world!").unwrap();
        let sub = path.join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("c.txt"), "nested").unwrap();

        let manifest = super::build_folder_manifest(&dir_path, 12345).unwrap();
        assert_eq!(manifest.transfer_id, 12345);
        assert_eq!(manifest.total_files, 3);
        assert_eq!(manifest.total_bytes, 5 + 6 + 6); // "hello" + "world!" + "nested"
        assert!(manifest.files.iter().any(|f| f.relative_path == "a.txt"));
        assert!(manifest.files.iter().any(|f| f.relative_path == "b.txt"));
        assert!(manifest.files.iter().any(|f| f.relative_path == "sub/c.txt"));

        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn test_build_folder_manifest_empty_dir() {
        let (dir_path, path) = create_temp_dir();
        assert!(super::build_folder_manifest(&dir_path, 1).is_none());
        let _ = std::fs::remove_dir_all(&path);
    }

    #[test]
    fn test_build_folder_manifest_nonexistent() {
        assert!(super::build_folder_manifest("/tmp/feiq_nonexistent_98765", 1).is_none());
    }

    #[test]
    fn test_folder_manifest_serialization_roundtrip() {
        use crate::protocol::types::{FolderFileEntry, FolderManifest};
        let manifest = FolderManifest {
            transfer_id: 42,
            folder_name: "test_folder".into(),
            total_files: 2,
            total_bytes: 100,
            files: vec![
                FolderFileEntry {
                    relative_path: "a.txt".into(),
                    size: 50,
                    modify_time: 1000,
                },
                FolderFileEntry {
                    relative_path: "sub/b.txt".into(),
                    size: 50,
                    modify_time: 2000,
                },
            ],
        };
        let json = serde_json::to_vec(&manifest).unwrap();
        let decoded: FolderManifest = serde_json::from_slice(&json).unwrap();
        assert_eq!(decoded.transfer_id, 42);
        assert_eq!(decoded.folder_name, "test_folder");
        assert_eq!(decoded.total_files, 2);
        assert_eq!(decoded.total_bytes, 100);
        assert_eq!(decoded.files.len(), 2);
        assert_eq!(decoded.files[0].relative_path, "a.txt");
        assert_eq!(decoded.files[1].relative_path, "sub/b.txt");
    }
}

/// Chunk size for file transfer (64KB for modern networks)
pub const FILE_CHUNK_SIZE: usize = 65536;

/// TCP file transfer server (waits for incoming file requests)
pub struct FileServer {
    listener: TcpListener,
}

impl FileServer {
    /// Start TCP server on the given port with SO_REUSEADDR enabled
    /// for multi-instance testing and quick restart support.
    pub async fn bind(port: u16) -> anyhow::Result<Self> {
        let addr: SocketAddr = format!("0.0.0.0:{port}").parse()?;
        let socket2_socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
        socket2_socket.set_reuse_address(true)?;
        socket2_socket.set_nonblocking(true)?;
        let sock_addr: socket2::SockAddr = addr.into();
        socket2_socket.bind(&sock_addr)?;
        socket2_socket.listen(1024)?;
        let listener = TcpListener::from_std(socket2_socket.into())?;
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
    /// If `cancel_flag` is provided and set to true, aborts early and returns
    /// a `Canceled` error.
    pub async fn send_file<F>(
        &mut self,
        file_path: &str,
        offset: i64,
        progress_cb: F,
        cancel_flag: Option<&AtomicBool>,
    ) -> anyhow::Result<i64>
    where
        F: Fn(i64, i64),
    {
        // Reject negative offset (would wrap to u64::MAX and seek past end)
        if offset < 0 {
            return Err(anyhow::anyhow!(
                "send_file: invalid negative offset {offset}"
            ));
        }

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
            // Check cancellation before each chunk
            if let Some(flag) = cancel_flag {
                if flag.load(Ordering::Relaxed) {
                    return Err(anyhow::anyhow!("send_file: canceled by user"));
                }
            }

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
    /// If `cancel_flag` is provided and set to true, aborts early and
    /// removes the partial file, returning a `Canceled` error.
    /// If the function returns an error (cancel, timeout, or disk failure),
    /// the partially written file is removed to prevent orphaned files.
    pub async fn recv_file<F>(
        &mut self,
        file_path: &str,
        total_size: i64,
        progress_cb: F,
        cancel_flag: Option<&AtomicBool>,
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

        let file_created = std::path::Path::new(file_path).exists();
        let mut file = tokio::fs::File::create(file_path).await?;
        let mut received: i64 = 0;
        let mut buf = vec![0u8; FILE_CHUNK_SIZE];

        let result: anyhow::Result<i64> = async {
            while received < total_size {
                // Check cancellation before each chunk
                if let Some(flag) = cancel_flag {
                    if flag.load(Ordering::Relaxed) {
                        return Err(anyhow::anyhow!("recv_file: canceled by user"));
                    }
                }

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
        }.await;

        // Clean up partial file on error (unless file was pre-existing)
        if result.is_err() && !file_created {
            let _ = std::fs::remove_file(file_path);
        }

        result
    }

    /// Receive raw bytes with timeout
    pub async fn recv(&mut self, buf: &mut [u8]) -> anyhow::Result<usize> {
        let n = self.stream.read(buf).await?;
        Ok(n)
    }

    // ─── Folder transfer protocol methods ────────────────────────

    /// Send the folder manifest as JSON with 4-byte LE u32 length prefix.
    pub async fn send_folder_manifest(
        &mut self,
        manifest: &FolderManifest,
    ) -> anyhow::Result<()> {
        let json = serde_json::to_vec(manifest)?;
        let len = json.len() as u32;
        self.stream.write_all(&len.to_le_bytes()).await?;
        self.stream.write_all(&json).await?;
        Ok(())
    }

    /// Receive the folder manifest (4-byte LE u32 length prefix + JSON body).
    /// Rejects manifests larger than 10 MiB to prevent OOM attacks.
    pub async fn recv_folder_manifest(&mut self) -> anyhow::Result<FolderManifest> {
        const MAX_MANIFEST_SIZE: usize = 10 * 1024 * 1024; // 10 MiB

        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_le_bytes(len_buf) as usize;

        if len > MAX_MANIFEST_SIZE {
            return Err(anyhow::anyhow!(
                "Manifest size {len} exceeds maximum {MAX_MANIFEST_SIZE}"
            ));
        }

        let mut json_buf = vec![0u8; len];
        self.stream.read_exact(&mut json_buf).await?;
        let manifest: FolderManifest = serde_json::from_slice(&json_buf)?;

        // Limit total files to prevent resource exhaustion
        if manifest.total_files > 50000 {
            return Err(anyhow::anyhow!(
                "Manifest total_files {} exceeds maximum 50000",
                manifest.total_files
            ));
        }

        Ok(manifest)
    }

    /// Send a per-file header before streaming file content.
    /// Format: 4-byte LE u32 path_len + UTF-8 path bytes + 8-byte LE u64 file_size.
    pub async fn send_folder_file_header(
        &mut self,
        relative_path: &str,
        file_size: u64,
    ) -> anyhow::Result<()> {
        let path_bytes = relative_path.as_bytes();
        let path_len = path_bytes.len() as u32;
        self.stream.write_all(&path_len.to_le_bytes()).await?;
        self.stream.write_all(path_bytes).await?;
        self.stream.write_all(&file_size.to_le_bytes()).await?;
        Ok(())
    }

    /// Receive a per-file header.
    /// Returns (relative_path, file_size).
    /// Rejects path lengths larger than 4096 to prevent OOM attacks.
    pub async fn recv_folder_file_header(&mut self) -> anyhow::Result<(String, u64)> {
        const MAX_PATH_LEN: usize = 4096;

        let mut path_len_buf = [0u8; 4];
        self.stream.read_exact(&mut path_len_buf).await?;
        let path_len = u32::from_le_bytes(path_len_buf) as usize;

        if path_len > MAX_PATH_LEN {
            return Err(anyhow::anyhow!(
                "Folder file path length {path_len} exceeds maximum {MAX_PATH_LEN}"
            ));
        }

        let mut path_buf = vec![0u8; path_len];
        self.stream.read_exact(&mut path_buf).await?;
        let relative_path = String::from_utf8(path_buf)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in folder file path: {e}"))?;

        let mut size_buf = [0u8; 8];
        self.stream.read_exact(&mut size_buf).await?;
        let file_size = u64::from_le_bytes(size_buf);

        Ok((relative_path, file_size))
    }

    /// Send a protocol marker (fixed byte string).
    pub async fn send_marker(&mut self, marker: &[u8]) -> anyhow::Result<()> {
        self.stream.write_all(marker).await?;
        Ok(())
    }

    /// Read exactly `marker.len()` bytes and compare against the expected marker.
    /// Uses a 30-second timeout to prevent indefinite blocking.
    pub async fn expect_marker(&mut self, marker: &[u8]) -> anyhow::Result<bool> {
        let mut buf = vec![0u8; marker.len()];
        match timeout(Duration::from_secs(30), self.stream.read_exact(&mut buf)).await {
            Ok(Ok(_)) => Ok(buf == marker),
            Ok(Err(_)) => Ok(false),
            Err(_) => Ok(false), // timeout
        }
    }
}

/// Build a `FolderManifest` by walking a directory tree.
/// Returns None if the path is not a readable directory or is empty.
pub fn build_folder_manifest(
    folder_path: &str,
    transfer_id: u64,
) -> Option<FolderManifest> {
    let metadata = std::fs::metadata(folder_path).ok()?;
    if !metadata.is_dir() {
        return None;
    }

    let folder_name = std::path::Path::new(folder_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let mut files = Vec::new();
    walk_directory(folder_path, "", &mut files)?;

    if files.is_empty() {
        return None;
    }

    let total_bytes: u64 = files.iter().map(|f| f.size).sum();

    Some(FolderManifest {
        transfer_id,
        folder_name,
        total_files: files.len() as u32,
        total_bytes,
        files,
    })
}

/// Recursively walk a directory, collecting file entries with relative paths.
fn walk_directory(base_path: &str, relative_prefix: &str, files: &mut Vec<FolderFileEntry>) -> Option<()> {
    let dir = std::fs::read_dir(base_path).ok()?;

    for entry in dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue, // skip unreadable entries instead of aborting
        };
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue, // skip non-UTF-8 filenames
        };

        // Skip hidden files
        if name.starts_with('.') {
            continue;
        }

        // Use symlink_metadata to detect symlinks without following them.
        // ALL symlinks are skipped to prevent traversal attacks and
        // infinite recursion from symlinks pointing to parent directories.
        let sym_meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue, // skip instead of aborting the entire walk
        };
        if sym_meta.file_type().is_symlink() {
            continue; // skip all symlinks
        }

        let meta = sym_meta; // not a symlink, use it directly

        let rel_path = if relative_prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", relative_prefix, name)
        };

        if meta.is_dir() {
            // Recursively walk subdirectories
            let sub_base = path.to_string_lossy().to_string();
            let _ = walk_directory(&sub_base, &rel_path, files); // skip unreadable subdirs
        } else if meta.is_file() {
            let modify_time = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            files.push(FolderFileEntry {
                relative_path: rel_path,
                size: meta.len(),
                modify_time,
            });
        }
        // Skip devices, symlink dirs (already handled above), etc.
    }

    Some(())
}
