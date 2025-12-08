//! Integration tests for HTTP downloader

use engine::HttpDownloader;
use tokio::fs;

#[tokio::test]
async fn test_download_small_file() {
    let downloader = HttpDownloader::new();
    
    // use httpbin.org as a reliable test endpoint (returns JSON with request info)
    let url = "https://httpbin.org/bytes/1024"; // download exactly 1KB
    
    // create temp directory for test
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("fluxdm_test_download.bin");
    
    // clean up any previous test file
    let _ = fs::remove_file(&file_path).await;
    
    // download the file
    let result = downloader.download(url, &file_path).await;
    
    assert!(result.is_ok(), "Download failed: {:?}", result);
    
    let bytes_downloaded = result.unwrap();
    assert_eq!(bytes_downloaded, 1024, "Should download exactly 1KB");
    
    // verify file exists
    assert!(file_path.exists(), "Downloaded file should exist");
    
    // verify file size
    let metadata = fs::metadata(&file_path).await.unwrap();
    assert_eq!(metadata.len(), 1024, "File size should be 1KB");
    
    // clean up
    fs::remove_file(&file_path).await.unwrap();
}

#[tokio::test]
async fn test_download_with_content_length() {
    let downloader = HttpDownloader::new();
    
    // test getting content length - use a static file that reliably returns content-length
    let url = "https://httpbin.org/image/png";
    
    let result = downloader.get_content_length(url).await;
    
    assert!(result.is_ok(), "Failed to get content length");
    
    let content_length = result.unwrap();
    // just verify we got some content length, don't check exact size
    assert!(content_length.is_some(), "Should have content length");
    assert!(content_length.unwrap() > 0, "Content length should be > 0");
}

#[tokio::test]
async fn test_download_404_error() {
    let downloader = HttpDownloader::new();
    
    // intentionally try to download non-existent file
    let url = "https://httpbin.org/status/404";
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("fluxdm_test_404.bin");
    
    let result = downloader.download(url, &file_path).await;
    
    assert!(result.is_err(), "Should fail with 404");
    
    // verify the error is HTTP 404
    if let Err(e) = result {
        assert!(matches!(e, engine::DownloadError::HttpError(404)));
    }
}
