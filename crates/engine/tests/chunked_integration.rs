//! Integration tests for chunked downloads

use engine::ChunkedDownloader;
use tokio::fs;

#[tokio::test]
#[ignore] // network test, run manually
async fn test_chunked_download() {
    let downloader = ChunkedDownloader::new();
    
    // download a ~10KB file (small but not too small)
    let url = "https://httpbin.org/bytes/10240";
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_chunked.bin");
    
    // clean up any existing file
    let _ = fs::remove_file(&file_path).await;
    
    // download file
    let result = downloader.download(url, &file_path).await;
    assert!(result.is_ok(), "Download failed: {:?}", result.err());
    
    let bytes_downloaded = result.unwrap();
    assert_eq!(bytes_downloaded, 10240, "Downloaded {} bytes, expected 10240", bytes_downloaded);
    
    // verify file exists and has correct size
    let metadata = fs::metadata(&file_path).await.expect("File not found");
    assert_eq!(metadata.len(), 10240);
    
    // cleanup
    fs::remove_file(&file_path).await.expect("Failed to cleanup");
}

#[tokio::test]
#[ignore] // network test
async fn test_supports_ranges() {
    let downloader = ChunkedDownloader::new();
    
    // test with httpbin (supports ranges)
    let result = downloader.supports_ranges("https://httpbin.org/bytes/1000").await;
    assert!(result.is_ok());
    
    // note: httpbin may or may not support ranges depending on deployment
    // this test just verifies the function works, not the specific result
    println!("httpbin supports ranges: {:?}", result.unwrap());
}

#[tokio::test]
#[ignore] // network test
async fn test_get_file_info() {
    let downloader = ChunkedDownloader::new();
    
    let result = downloader.get_file_info("https://httpbin.org/bytes/5000").await;
    assert!(result.is_ok());
    
    let (size, supports_ranges) = result.unwrap();
    assert_eq!(size, 5000);
    
    println!("File size: {}, Supports ranges: {}", size, supports_ranges);
}

#[tokio::test]
async fn test_chunk_calculation_integration() {
    let downloader = ChunkedDownloader::new();
    
    // test with 8MB file (should split into 8 chunks)
    let file_size = 8_388_608; // 8MB
    let chunks = downloader.calculate_chunks(file_size);
    
    assert_eq!(chunks.len(), 8);
    
    // verify all chunks cover the entire file
    assert_eq!(chunks[0].start, 0);
    assert_eq!(chunks[7].end, file_size - 1);
    
    // verify chunks are contiguous (no gaps)
    for i in 0..chunks.len() - 1 {
        assert_eq!(chunks[i].end + 1, chunks[i + 1].start);
    }
    
    // verify total size matches
    let total_size: u64 = chunks.iter().map(|c| c.size()).sum();
    assert_eq!(total_size, file_size);
}
