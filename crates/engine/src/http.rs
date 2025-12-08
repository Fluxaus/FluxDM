//! HTTP download functionality

use reqwest::Client;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// HTTP downloader for single-threaded downloads
pub struct HttpDownloader {
    client: Client,
}

impl HttpDownloader {
    /// Creates a new HTTP downloader
    pub fn new() -> Self {
        let client = Client::builder()
            .user_agent("FluxDM/0.1.0")
            .build()
            .expect("failed to create HTTP client"); // temporary, will improve error handling

        Self { client }
    }

    /// Downloads a file from URL to the specified path
    ///
    /// # Arguments
    ///
    /// * `url` - The URL to download from
    /// * `path` - Destination file path
    ///
    /// # Returns
    ///
    /// Returns the total number of bytes downloaded
    pub async fn download(&self, url: &str, path: &Path) -> Result<u64, DownloadError> {
        // make the HTTP request
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

        // check if request was successful
        if !response.status().is_success() {
            return Err(DownloadError::HttpError(response.status().as_u16()));
        }

        // get content length if available (for future progress tracking)
        let _total_size = response.content_length();

        // create the output file
        let mut file = File::create(path)
            .await
            .map_err(|e| DownloadError::FileError(e.to_string()))?;

        // stream the response body to file
        let mut bytes_downloaded = 0u64;
        let mut stream = response.bytes_stream();

        use futures_util::StreamExt; // for .next()

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| DownloadError::NetworkError(e.to_string()))?;

            file.write_all(&chunk)
                .await
                .map_err(|e| DownloadError::FileError(e.to_string()))?;

            bytes_downloaded += chunk.len() as u64;
        }

        // ensure all data is written to disk
        file.flush()
            .await
            .map_err(|e| DownloadError::FileError(e.to_string()))?;

        Ok(bytes_downloaded)
    }

    /// Gets the content length of a URL without downloading
    pub async fn get_content_length(&self, url: &str) -> Result<Option<u64>, DownloadError> {
        let response = self
            .client
            .head(url)
            .send()
            .await
            .map_err(|e| DownloadError::NetworkError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DownloadError::HttpError(response.status().as_u16()));
        }

        Ok(response.content_length())
    }
}

impl Default for HttpDownloader {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during download
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    /// Network-related error
    NetworkError(String),
    /// HTTP error with status code
    HttpError(u16),
    /// File I/O error
    FileError(String),
    /// Invalid URL
    InvalidUrl(String),
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            DownloadError::HttpError(code) => write!(f, "HTTP error: {}", code),
            DownloadError::FileError(msg) => write!(f, "File error: {}", msg),
            DownloadError::InvalidUrl(msg) => write!(f, "Invalid URL: {}", msg),
        }
    }
}

impl std::error::Error for DownloadError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_downloader_creation() {
        let _downloader = HttpDownloader::new();
        // just verify it doesn't panic
    }

    // note: actual download tests require network access
    // we'll add integration tests later with mock servers
}
