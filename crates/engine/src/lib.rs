/// Download status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadStatus {
    /// Queued
    Pending,
    /// In Progress
    Downloading,
    /// Paused
    Paused,
    /// Completed
    Completed,
    /// Failed
    Failed,
}

/// Basic struct of file download
pub struct Download {
    url: String,
    status: DownloadStatus,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
}

impl Download {
    /// Take URL as parameter
    pub fn new(url: String) -> Self {
        Self {
            url,
            status: DownloadStatus::Pending,
            bytes_downloaded: 0,
            total_bytes: None,
        }
    }

    /// Returns download's URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns download's current status
    pub fn status(&self) -> DownloadStatus {
        self.status
    }

    /// Returns the number of bytes downloaded so far
    pub fn bytes_downloaded(&self) -> u64 {
        self.bytes_downloaded
    }

    /// Returns the total file size in bytes, if known
    pub fn total_bytes(&self) -> Option<u64> {
        self.total_bytes
    }

    /// Returns the download progress as a percentage (0.0 to 100.0)
    pub fn progress_percent(&self) -> f64 {
        match self.total_bytes {
            Some(total) if total > 0 => (self.bytes_downloaded as f64 / total as f64) * 100.0,
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_creation() {
        // Test creating a new download with a URL
        let url = "https://example.com/file.zip";
        let download = Download::new(url.to_string());
        
        assert_eq!(download.url(), url);
    }

    #[test]
    fn test_download_initial_status() {
        // New downloads should start in Pending status
        let download = Download::new("https://example.com/file.zip".to_string());
        assert_eq!(download.status(), DownloadStatus::Pending);
    }

    #[test]
    fn test_download_progress() {
        // New downloads should have zero progress
        let download = Download::new("https://example.com/file.zip".to_string());
        assert_eq!(download.bytes_downloaded(), 0);
        assert_eq!(download.total_bytes(), None); // Unknown until we start
        assert_eq!(download.progress_percent(), 0.0);
    }
}
