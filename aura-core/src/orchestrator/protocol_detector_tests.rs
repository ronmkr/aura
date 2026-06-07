use crate::orchestrator::protocol_detector::{DetectedType, ProtocolDetector};
use tempfile::tempdir;
use tokio::fs;

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
    // 40-char Hex (v1)
    assert_eq!(
        ProtocolDetector::detect("dca7c79e604f3261621217e472658b191c03975a").await,
        Some(DetectedType::BitTorrent)
    );

    // 64-char Hex (v2)
    assert_eq!(
        ProtocolDetector::detect(
            "dca7c79e604f3261621217e472658b191c03975adca7c79e604f3261621217e4"
        )
        .await,
        Some(DetectedType::BitTorrent)
    );

    // 32-char Base32 (v1)
    assert_eq!(
        ProtocolDetector::detect("33T4PH3AJ4ZGCSQSCHEHEXTLCGFAHF22").await,
        Some(DetectedType::BitTorrent)
    );
}

#[tokio::test]
async fn test_detect_extensions_fallback() {
    let dir = tempdir().unwrap();

    // .torrent file
    let torrent_path = dir.path().join("test.torrent");
    fs::write(&torrent_path, b"d8:announce3:url e")
        .await
        .unwrap();
    assert_eq!(
        ProtocolDetector::detect(torrent_path.to_str().unwrap()).await,
        Some(DetectedType::BitTorrent)
    );

    // .metalink file
    let metalink_path = dir.path().join("test.metalink");
    fs::write(
        &metalink_path,
        b"<?xml version=\"1.0\" encoding=\"utf-8\"?><metalink/>",
    )
    .await
    .unwrap();
    assert_eq!(
        ProtocolDetector::detect(metalink_path.to_str().unwrap()).await,
        Some(DetectedType::Metalink)
    );

    // Unknown file with .torrent extension
    let unknown_torrent = dir.path().join("fake.torrent");
    fs::write(&unknown_torrent, b"not a torrent").await.unwrap();
    assert_eq!(
        ProtocolDetector::detect(unknown_torrent.to_str().unwrap()).await,
        Some(DetectedType::BitTorrent)
    );

    // Unknown file with .metalink extension
    let unknown_metalink = dir.path().join("fake.metalink");
    fs::write(&unknown_metalink, b"not a metalink")
        .await
        .unwrap();
    assert_eq!(
        ProtocolDetector::detect(unknown_metalink.to_str().unwrap()).await,
        Some(DetectedType::Metalink)
    );

    // Just extension-based detection (no file exists)
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
