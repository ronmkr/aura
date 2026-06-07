use super::*;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_detect_uri_schemes() {
    assert_eq!(
        ProtocolDetector::detect("http://example.com").await,
        Some(DetectedType::Http)
    );
    assert_eq!(
        ProtocolDetector::detect("https://example.com").await,
        Some(DetectedType::Https)
    );
    assert_eq!(
        ProtocolDetector::detect("ftp://example.com").await,
        Some(DetectedType::Ftp)
    );
    assert_eq!(
        ProtocolDetector::detect("ftps://example.com").await,
        Some(DetectedType::Ftps)
    );
    assert_eq!(
        ProtocolDetector::detect("magnet:?xt=urn:btih:abc").await,
        Some(DetectedType::BitTorrent)
    );
}

#[tokio::test]
async fn test_detect_info_hashes() {
    // v1 hex
    assert_eq!(
        ProtocolDetector::detect("dca7c79e604f3261621217e472658b191c03975a").await,
        Some(DetectedType::BitTorrent)
    );
    // v2 hex
    assert_eq!(
        ProtocolDetector::detect(
            "dca7c79e604f3261621217e472658b191c03975adca7c79e604f3261621217e4"
        )
        .await,
        Some(DetectedType::BitTorrent)
    );
    // v1 base32
    assert_eq!(
        ProtocolDetector::detect("33T4PH3AJ4ZGCSQSCHEHEXTLCGFAHF22").await,
        Some(DetectedType::BitTorrent)
    );
}

#[tokio::test]
async fn test_detect_local_files() {
    let dir = tempdir().unwrap();

    // Torrent file by extension
    let torrent_path = dir.path().join("test.torrent");
    fs::write(&torrent_path, "d8:announce0:e").unwrap();
    assert_eq!(
        ProtocolDetector::detect(torrent_path.to_str().unwrap()).await,
        Some(DetectedType::BitTorrent)
    );

    // Metalink file by extension
    let metalink_path = dir.path().join("test.metalink");
    fs::write(
        &metalink_path,
        "<?xml version=\"1.0\"?><metalink></metalink>",
    )
    .unwrap();
    assert_eq!(
        ProtocolDetector::detect(metalink_path.to_str().unwrap()).await,
        Some(DetectedType::Metalink)
    );

    // Torrent file by content (no extension)
    let unknown_torrent = dir.path().join("unknown_bt");
    fs::write(&unknown_torrent, "d8:announce3:url7:comment5:helloe").unwrap();
    assert_eq!(
        ProtocolDetector::detect(unknown_torrent.to_str().unwrap()).await,
        Some(DetectedType::BitTorrent)
    );

    // Metalink file by content (no extension)
    let unknown_metalink = dir.path().join("unknown_ml");
    fs::write(&unknown_metalink, "<metalink version=\"4.0\"></metalink>").unwrap();
    assert_eq!(
        ProtocolDetector::detect(unknown_metalink.to_str().unwrap()).await,
        Some(DetectedType::Metalink)
    );
}

#[tokio::test]
async fn test_detect_extensions_fallback() {
    assert_eq!(
        ProtocolDetector::detect("some_file.torrent").await,
        Some(DetectedType::BitTorrent)
    );
    assert_eq!(
        ProtocolDetector::detect("some_file.metalink").await,
        Some(DetectedType::Metalink)
    );
    assert_eq!(
        ProtocolDetector::detect("some_file.meta4").await,
        Some(DetectedType::Metalink)
    );
}

#[tokio::test]
async fn test_detect_unknown() {
    assert_eq!(ProtocolDetector::detect("random_string").await, None);
    assert_eq!(ProtocolDetector::detect("").await, None);
}
