//! Integration tests for HTTP downloader
//!
//! These tests require network access and use external services.
//! Run with: cargo test -p engine --test http_integration -- --ignored

use engine::HttpDownloader;
use tokio::fs;

#[tokio::test]
#[ignore] // requires network, may be flaky
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
#[ignore] // requires network, may be flaky
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
