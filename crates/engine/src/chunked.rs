//! Multi-part (chunked) download implementation

use crate::DownloadError;
use reqwest::Client;
use std::path::Path;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::time::sleep;

/// Configuration for chunked downloads
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Number of chunks to split the file into
    pub chunk_count: u8,
    /// Minimum size in bytes for a chunk (don't split if file is smaller)
    pub min_chunk_size: u64,
    /// Maximum number of retry attempts per chunk
    pub max_retries: u32,
    /// Initial retry delay in milliseconds
    pub retry_delay_ms: u64,
    /// Whether to use exponential backoff (doubles delay each retry)
    pub exponential_backoff: bool,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_count: 8,               // 8 parallel connections (like IDM)
            min_chunk_size: 1_048_576,    // 1MB minimum per chunk
            max_retries: 3,               // retry up to 3 times
            retry_delay_ms: 1000,         // start with 1 second delay
            exponential_backoff: true,    // 1s, 2s, 4s, 8s...
        }
    }
}

/// Represents a single chunk of a file to download
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    /// Chunk index (0-based)
    pub index: u8,
    /// Starting byte position (inclusive)
    pub start: u64,
    /// Ending byte position (inclusive)
    pub end: u64,
    /// Bytes already downloaded for this chunk
    pub downloaded: u64,
}

impl Chunk {
    /// Returns the size of this chunk in bytes
    pub fn size(&self) -> u64 {
        self.end - self.start + 1
    }

    /// Returns the number of bytes remaining to download
    pub fn remaining(&self) -> u64 {
        self.size() - self.downloaded
    }

    /// Returns true if this chunk is complete
    pub fn is_complete(&self) -> bool {
        self.downloaded >= self.size()
    }

    /// Returns the next byte position to download from
    pub fn resume_position(&self) -> u64 {
        self.start + self.downloaded
    }
}

/// Chunked downloader for multi-part downloads
pub struct ChunkedDownloader {
    client: Client,
    config: ChunkConfig,
}

impl ChunkedDownloader {
    /// Creates a new chunked downloader with default config
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("FluxDM/0.1.0")
            .build()
            .expect("failed to create HTTP client"); // temporary
        
        Self {
            client,
            config: ChunkConfig::default(),
        }
    }

    /// Creates a new chunked downloader with custom config
    pub fn with_config(config: ChunkConfig) -> Self {
        let client = Client::builder()
            .user_agent("FluxDM/0.1.0")
            .build()
            .expect("failed to create HTTP client");
        
        Self { client, config }
    }

    /// Checks if the server supports Range requests
    pub async fn supports_ranges(&self, url: &str) -> Result<bool, DownloadError> {
        let response = self.client
            .head(url)
            .send()
            .await
            .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DownloadError::HttpError(response.status().as_u16()));
        }

        // check for Accept-Ranges header
        Ok(response
            .headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "bytes")
            .unwrap_or(false))
    }

    /// Gets the content length and whether ranges are supported
    pub async fn get_file_info(&self, url: &str) -> Result<(u64, bool), DownloadError> {
        let response = self.client
            .head(url)
            .send()
            .await
            .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DownloadError::HttpError(response.status().as_u16()));
        }

        let content_length = response
            .content_length()
            .ok_or_else(|| DownloadError::InvalidUrl("No content length".to_string()))?;

        let supports_ranges = response
            .headers()
            .get("accept-ranges")
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "bytes")
            .unwrap_or(false);

        Ok((content_length, supports_ranges))
    }

    /// Calculates optimal chunks for a file
    pub fn calculate_chunks(&self, file_size: u64) -> Vec<Chunk> {
        // if file is too small, use single chunk
        if file_size < self.config.min_chunk_size {
            return vec![Chunk {
                index: 0,
                start: 0,
                end: file_size - 1,
                downloaded: 0,
            }];
        }

        // calculate chunk size
        let chunk_size = file_size / self.config.chunk_count as u64;
        
        let mut chunks = Vec::new();
        let mut start = 0u64;

        for i in 0..self.config.chunk_count {
            let end = if i == self.config.chunk_count - 1 {
                file_size - 1 // last chunk gets remainder
            } else {
                start + chunk_size - 1
            };

            chunks.push(Chunk {
                index: i,
                start,
                end,
                downloaded: 0,
            });

            start = end + 1;
        }

        chunks
    }

    /// Detects if a partial file exists and updates chunks with already-downloaded bytes
    pub async fn detect_resume(
        &self,
        path: &Path,
        file_size: u64,
    ) -> Result<Vec<Chunk>, DownloadError> {
        // calculate chunks as if starting fresh
        let mut chunks = self.calculate_chunks(file_size);

        // check if file exists
        let metadata = match tokio::fs::metadata(path).await {
            Ok(m) => m,
            Err(_) => return Ok(chunks), // no file exists, start fresh
        };

        let existing_size = metadata.len();

        // if file is already the correct size, all chunks are done
        if existing_size >= file_size {
            for chunk in &mut chunks {
                chunk.downloaded = chunk.size();
            }
            return Ok(chunks);
        }

        // update chunks based on existing file size
        // we assume sequential writing from the start (simple case)
        let mut remaining = existing_size;
        
        for chunk in &mut chunks {
            let chunk_size = chunk.size();
            
            if remaining >= chunk_size {
                // entire chunk is downloaded
                chunk.downloaded = chunk_size;
                remaining -= chunk_size;
            } else if remaining > 0 {
                // partial chunk downloaded
                chunk.downloaded = remaining;
                remaining = 0;
            } else {
                // no data for this chunk yet
                break;
            }
        }

        Ok(chunks)
    }

    /// Downloads a single chunk with retry logic and exponential backoff
    async fn download_chunk_with_retry(
        &self,
        url: &str,
        chunk: Chunk,
        file: &mut File,
    ) -> Result<u64, DownloadError> {
        let mut attempt = 0;
        let mut last_error = None;

        loop {
            match self.download_chunk(url, chunk, file).await {
                Ok(bytes) => return Ok(bytes),
                Err(e) => {
                    last_error = Some(e);
                    attempt += 1;

                    // check if we've exhausted retries
                    if attempt > self.config.max_retries {
                        break;
                    }

                    // calculate backoff delay
                    let delay = if self.config.exponential_backoff {
                        // exponential: 1s, 2s, 4s, 8s...
                        self.config.retry_delay_ms * 2u64.pow(attempt - 1)
                    } else {
                        // constant delay
                        self.config.retry_delay_ms
                    };

                    // wait before retrying
                    sleep(Duration::from_millis(delay)).await;
                }
            }
        }

        // return the last error after exhausting retries
        Err(last_error.unwrap_or_else(|| {
            DownloadError::NetworkError("Unknown error during retry".to_string())
        }))
    }

    /// Downloads a single chunk and writes it to the file at the correct position
    /// Supports resuming from chunk.downloaded bytes
    async fn download_chunk(
        &self,
        url: &str,
        chunk: Chunk,
        file: &mut File,
    ) -> Result<u64, DownloadError> {
        // skip if chunk is already complete
        if chunk.is_complete() {
            return Ok(0);
        }

        // calculate range to download (resume from where we left off)
        let start_byte = chunk.resume_position();
        let end_byte = chunk.end;
        let range_header = format!("bytes={}-{}", start_byte, end_byte);

        let response = self.client
            .get(url)
            .header("Range", range_header)
            .send()
            .await
            .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

        // check for 206 Partial Content or 200 OK (some servers)
        if !response.status().is_success() && response.status().as_u16() != 206 {
            return Err(DownloadError::HttpError(response.status().as_u16()));
        }

        // seek to resume position in file
        file.seek(std::io::SeekFrom::Start(start_byte))
            .await
            .map_err(|e| DownloadError::FileError(e.to_string()))?;

        // stream chunk to file
        let mut bytes_written = 0u64;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;

        while let Some(chunk_data) = stream.next().await {
            let chunk_data = chunk_data.map_err(|e| DownloadError::NetworkError(e.to_string()))?;
            
            file.write_all(&chunk_data)
                .await
                .map_err(|e| DownloadError::FileError(e.to_string()))?;
            
            bytes_written += chunk_data.len() as u64;
        }

        Ok(bytes_written)
    }

    /// Downloads a file using multiple parallel chunks
    pub async fn download(
        &self,
        url: &str,
        path: &Path,
    ) -> Result<u64, DownloadError> {
        // get file info
        let (file_size, supports_ranges) = self.get_file_info(url).await?;

        // if ranges not supported, fall back to single download
        if !supports_ranges {
            return self.download_single(url, path).await;
        }

        // calculate chunks
        let chunks = self.calculate_chunks(file_size);

        // create output file with correct size (pre-allocate)
        let file = File::create(path)
            .await
            .map_err(|e| DownloadError::FileError(e.to_string()))?;
        
        file.set_len(file_size)
            .await
            .map_err(|e| DownloadError::FileError(e.to_string()))?;

        // download chunks in parallel with retry logic
        let mut tasks = Vec::new();
        
        for chunk in chunks {
            let url = url.to_string();
            let path = path.to_path_buf();
            let client = self.client.clone();
            let config = self.config.clone();

            let task = tokio::spawn(async move {
                let downloader = Self {
                    client,
                    config,
                };
                
                let mut file = File::options()
                    .write(true)
                    .open(&path)
                    .await
                    .map_err(|e| DownloadError::FileError(e.to_string()))?;

                downloader.download_chunk_with_retry(&url, chunk, &mut file).await
            });

            tasks.push(task);
        }

        // wait for all chunks to complete
        let mut total_bytes = 0u64;
        
        for task in tasks {
            let bytes = task
                .await
                .map_err(|e| DownloadError::NetworkError(format!("Task failed: {}", e)))?
                ?;
            
            total_bytes += bytes;
        }

        Ok(total_bytes)
    }

    /// Downloads a file with resume support (detects partial files)
    pub async fn download_resumable(
        &self,
        url: &str,
        path: &Path,
    ) -> Result<u64, DownloadError> {
        // get file info
        let (file_size, supports_ranges) = self.get_file_info(url).await?;

        // if ranges not supported, fall back to single download
        if !supports_ranges {
            return self.download_single(url, path).await;
        }

        // detect existing partial file and get chunks with resume info
        let chunks = self.detect_resume(path, file_size).await?;

        // check if download is already complete
        let total_remaining: u64 = chunks.iter().map(|c| c.remaining()).sum();
        if total_remaining == 0 {
            return Ok(0); // already complete
        }

        // ensure file exists with correct size
        let file = if tokio::fs::metadata(path).await.is_ok() {
            // file exists, open for writing
            File::options()
                .write(true)
                .open(path)
                .await
                .map_err(|e| DownloadError::FileError(e.to_string()))?
        } else {
            // create new file with correct size
            let file = File::create(path)
                .await
                .map_err(|e| DownloadError::FileError(e.to_string()))?;
            
            file.set_len(file_size)
                .await
                .map_err(|e| DownloadError::FileError(e.to_string()))?;
            
            file
        };

        // close the file handle, we'll reopen in each task
        drop(file);

        // download chunks in parallel (only incomplete ones)
        let mut tasks = Vec::new();
        
        for chunk in chunks {
            // skip complete chunks
            if chunk.is_complete() {
                continue;
            }

            let url = url.to_string();
            let path = path.to_path_buf();
            let client = self.client.clone();
            let config = self.config.clone();

            let task = tokio::spawn(async move {
                let downloader = Self {
                    client,
                    config,
                };
                
                let mut file = File::options()
                    .write(true)
                    .open(&path)
                    .await
                    .map_err(|e| DownloadError::FileError(e.to_string()))?;

                downloader.download_chunk_with_retry(&url, chunk, &mut file).await
            });

            tasks.push(task);
        }

        // wait for all chunks to complete
        let mut total_bytes = 0u64;
        
        for task in tasks {
            let bytes = task
                .await
                .map_err(|e| DownloadError::NetworkError(format!("Task failed: {}", e)))?
                ?;
            
            total_bytes += bytes;
        }

        Ok(total_bytes)
    }

    /// Fallback to single-threaded download
    async fn download_single(&self, url: &str, path: &Path) -> Result<u64, DownloadError> {
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DownloadError::HttpError(response.status().as_u16()));
        }

        let mut file = File::create(path)
            .await
            .map_err(|e| DownloadError::FileError(e.to_string()))?;

        let mut bytes_downloaded = 0u64;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| DownloadError::NetworkError(e.to_string()))?;
            
            file.write_all(&chunk)
                .await
                .map_err(|e| DownloadError::FileError(e.to_string()))?;
            
            bytes_downloaded += chunk.len() as u64;
        }

        file.flush()
            .await
            .map_err(|e| DownloadError::FileError(e.to_string()))?;

        Ok(bytes_downloaded)
    }
}

impl Default for ChunkedDownloader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_calculation() {
        let config = ChunkConfig {
            chunk_count: 4,
            min_chunk_size: 100,
        };
        let downloader = ChunkedDownloader::with_config(config);

        // test file split into 4 chunks
        let chunks = downloader.calculate_chunks(1000);
        assert_eq!(chunks.len(), 4);
        
        // verify chunks are contiguous
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 249);
        assert_eq!(chunks[1].start, 250);
        assert_eq!(chunks[1].end, 499);
        assert_eq!(chunks[2].start, 500);
        assert_eq!(chunks[2].end, 749);
        assert_eq!(chunks[3].start, 750);
        assert_eq!(chunks[3].end, 999);
    }

    #[test]
    fn test_small_file_single_chunk() {
        let config = ChunkConfig {
            chunk_count: 8,
            min_chunk_size: 1_000_000,
        };
        let downloader = ChunkedDownloader::with_config(config);

        // file smaller than min_chunk_size should be single chunk
        let chunks = downloader.calculate_chunks(500_000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].start, 0);
        assert_eq!(chunks[0].end, 499_999);
    }

    #[test]
    fn test_chunk_size_calculation() {
        let chunk = Chunk {
            index: 0,
            start: 100,
            end: 199,
            downloaded: 0,
        };
        assert_eq!(chunk.size(), 100);
    }

    #[test]
    fn test_chunk_resume_tracking() {
        let mut chunk = Chunk {
            index: 0,
            start: 0,
            end: 999,
            downloaded: 500,
        };
        
        assert_eq!(chunk.size(), 1000);
        assert_eq!(chunk.remaining(), 500);
        assert_eq!(chunk.resume_position(), 500);
        assert!(!chunk.is_complete());
        
        // simulate completing the chunk
        chunk.downloaded = 1000;
        assert_eq!(chunk.remaining(), 0);
        assert!(chunk.is_complete());
    }

    #[tokio::test]
    async fn test_resume_detection_no_file() {
        let downloader = ChunkedDownloader::new();
        let path = std::path::PathBuf::from("/nonexistent/file.bin");
        
        let chunks = downloader.detect_resume(&path, 8_000_000).await.unwrap();
        
        // should return fresh chunks (all with downloaded=0)
        assert_eq!(chunks.len(), 8);
        for chunk in chunks {
            assert_eq!(chunk.downloaded, 0);
            assert!(!chunk.is_complete());
        }
    }

    #[tokio::test]
    async fn test_resume_detection_partial_file() {
        use tokio::io::AsyncWriteExt;
        
        let downloader = ChunkedDownloader::new();
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_resume_partial.bin");
        
        // clean up any existing file
        let _ = tokio::fs::remove_file(&file_path).await;
        
        // create partial file (2.5MB of an 8MB file)
        let partial_size = 2_621_440u64; // 2.5MB
        let mut file = File::create(&file_path).await.unwrap();
        let data = vec![0u8; partial_size as usize];
        file.write_all(&data).await.unwrap();
        file.flush().await.unwrap();
        drop(file);
        
        // detect resume
        let total_size = 8_388_608u64; // 8MB
        let chunks = downloader.detect_resume(&file_path, total_size).await.unwrap();
        
        // verify chunks
        assert_eq!(chunks.len(), 8);
        
        // each chunk is 1MB (1_048_576 bytes)
        // partial_size is 2.5MB, so first 2 chunks complete, 3rd chunk half done
        assert_eq!(chunks[0].downloaded, 1_048_576); // complete
        assert_eq!(chunks[1].downloaded, 1_048_576); // complete
        assert_eq!(chunks[2].downloaded, 524_288);   // 0.5MB done
        assert_eq!(chunks[3].downloaded, 0);         // not started
        
        assert!(chunks[0].is_complete());
        assert!(chunks[1].is_complete());
        assert!(!chunks[2].is_complete());
        assert!(!chunks[3].is_complete());
        
        // cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[tokio::test]
    async fn test_resume_detection_complete_file() {
        use tokio::io::AsyncWriteExt;
        
        let downloader = ChunkedDownloader::new();
        let temp_dir = std::env::temp_dir();
        let file_path = temp_dir.join("test_resume_complete.bin");
        
        // clean up any existing file
        let _ = tokio::fs::remove_file(&file_path).await;
        
        // create complete file (8MB)
        let file_size = 8_388_608u64;
        let mut file = File::create(&file_path).await.unwrap();
        let data = vec![0u8; file_size as usize];
        file.write_all(&data).await.unwrap();
        file.flush().await.unwrap();
        drop(file);
        
        // detect resume
        let chunks = downloader.detect_resume(&file_path, file_size).await.unwrap();
        
        // all chunks should be complete
        assert_eq!(chunks.len(), 8);
        for chunk in chunks {
            assert!(chunk.is_complete());
            assert_eq!(chunk.remaining(), 0);
        }
        
        // cleanup
        let _ = tokio::fs::remove_file(&file_path).await;
    }

    #[test]
    fn test_retry_config() {
        let config = ChunkConfig::default();
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert!(config.exponential_backoff);
    }

    #[test]
    fn test_custom_retry_config() {
        let config = ChunkConfig {
            chunk_count: 4,
            min_chunk_size: 512_000,
            max_retries: 5,
            retry_delay_ms: 500,
            exponential_backoff: false,
        };

        let downloader = ChunkedDownloader::with_config(config.clone());
        assert_eq!(downloader.config.max_retries, 5);
        assert_eq!(downloader.config.retry_delay_ms, 500);
        assert!(!downloader.config.exponential_backoff);
    }

    #[test]
    fn test_exponential_backoff_delays() {
        // simulate exponential backoff calculation
        let base_delay = 1000u64;
        
        // attempt 1: 1s (2^0 = 1)
        let delay1 = base_delay * 2u64.pow(0);
        assert_eq!(delay1, 1000);
        
        // attempt 2: 2s (2^1 = 2)
        let delay2 = base_delay * 2u64.pow(1);
        assert_eq!(delay2, 2000);
        
        // attempt 3: 4s (2^2 = 4)
        let delay3 = base_delay * 2u64.pow(2);
        assert_eq!(delay3, 4000);
        
        // attempt 4: 8s (2^3 = 8)
        let delay4 = base_delay * 2u64.pow(3);
        assert_eq!(delay4, 8000);
    }
}
