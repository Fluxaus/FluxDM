use std::path::PathBuf;
use std::time::SystemTime;

/// Unique identifier for a download
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DownloadId(u64);

impl DownloadId {
    /// Creates a new download ID
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    /// Returns the inner ID value
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

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
    id: DownloadId,
    url: String,
    file_path: Option<PathBuf>,
    status: DownloadStatus,
    bytes_downloaded: u64,
    total_bytes: Option<u64>,
    created_at: SystemTime,
    started_at: Option<SystemTime>,
    completed_at: Option<SystemTime>,
    error_message: Option<String>,
}

impl Download {
    /// Take URL as parameter
    pub fn new(id: DownloadId, url: String) -> Self {
        Self {
            id,
            url,
            file_path: None,
            status: DownloadStatus::Pending,
            bytes_downloaded: 0,
            total_bytes: None,
            created_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }

    /// Returns the download's unique ID
    pub fn id(&self) -> DownloadId {
        self.id
    }

    /// Returns download's URL
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Returns the file path where download will be saved
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    /// Sets the file path for this download
    pub fn set_file_path(&mut self, path: PathBuf) {
        self.file_path = Some(path);
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

    /// Returns when the download was created
    pub fn created_at(&self) -> SystemTime {
        self.created_at
    }

    /// Returns when the download started, if it has started
    pub fn started_at(&self) -> Option<SystemTime> {
        self.started_at
    }

    /// Returns when the download completed, if it has completed
    pub fn completed_at(&self) -> Option<SystemTime> {
        self.completed_at
    }

    /// Returns the error message if download failed
    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_creation() {
        // Test creating a new download with a URL
        let url = "https://example.com/file.zip";
        let id = DownloadId::new(1);
        let download = Download::new(id, url.to_string());

        assert_eq!(download.id(), id);
        assert_eq!(download.url(), url);
        assert_eq!(download.file_path(), None);
    }

    #[test]
    fn test_download_initial_status() {
        // New downloads should start in Pending status
        let id = DownloadId::new(2);
        let download = Download::new(id, "https://example.com/file.zip".to_string());
        assert_eq!(download.status(), DownloadStatus::Pending);
    }

    #[test]
    fn test_download_progress() {
        // New downloads should have zero progress
        let id = DownloadId::new(3);
        let download = Download::new(id, "https://example.com/file.zip".to_string());
        assert_eq!(download.bytes_downloaded(), 0);
        assert_eq!(download.total_bytes(), None); // Unknown until we start
        assert_eq!(download.progress_percent(), 0.0);
    }

    #[test]
    fn test_download_file_path() {
        // Test setting and getting file path
        let id = DownloadId::new(4);
        let mut download = Download::new(id, "https://example.com/file.zip".to_string());

        assert_eq!(download.file_path(), None);

        let path = PathBuf::from("/downloads/file.zip");
        download.set_file_path(path.clone());

        assert_eq!(download.file_path(), Some(&path));
    }

    #[test]
    fn test_download_timestamps() {
        // Test that created_at is set on creation
        let id = DownloadId::new(5);
        let download = Download::new(id, "https://example.com/file.zip".to_string());

        // created_at should be recent (within last second)
        let now = SystemTime::now();
        let created = download.created_at();
        assert!(now.duration_since(created).unwrap().as_secs() < 1);

        // started_at and completed_at should be None for new download
        assert_eq!(download.started_at(), None);
        assert_eq!(download.completed_at(), None);
}
