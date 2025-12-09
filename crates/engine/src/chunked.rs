//! Multi-part (chunked) download implementation

use crate::DownloadError;
use reqwest::Client;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};

/// Configuration for chunked downloads
#[derive(Debug, Clone)]
pub struct ChunkConfig {
    /// Number of chunks to split the file into
    pub chunk_count: u8,
    /// Minimum size in bytes for a chunk (don't split if file is smaller)
    pub min_chunk_size: u64,
}

impl Default for ChunkConfig {
    fn default() -> Self {
        Self {
            chunk_count: 8,        // 8 parallel connections (like IDM)
            min_chunk_size: 1_048_576, // 1MB minimum per chunk
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

    /// Downloads a single chunk and writes it to the file at the correct position
    async fn download_chunk(
        &self,
        url: &str,
        chunk: Chunk,
        file: &mut File,
    ) -> Result<u64, DownloadError> {
        // build Range header: "bytes=start-end"
        let range_header = format!("bytes={}-{}", chunk.start, chunk.end);

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

        // seek to correct position in file
        file.seek(std::io::SeekFrom::Start(chunk.start))
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

        // download chunks in parallel
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

                downloader.download_chunk(&url, chunk, &mut file).await
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
        };
        assert_eq!(chunk.size(), 100);
    }
}
