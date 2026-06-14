use super::connection::parse_ybegin;
use super::worker::NntpWorker;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn test_ybegin_parsing() {
    let line1 = "=ybegin line=128 size=123456 name=test_file.bin";
    let (name, size) = parse_ybegin(line1).expect("Failed to parse line1");
    assert_eq!(name, "test_file.bin");
    assert_eq!(size, 123456);

    let line2 =
        "=ybegin part=1 total=10 line=128 size=987654 name=another file name with spaces.zip";
    let (name, size) = parse_ybegin(line2).expect("Failed to parse line2");
    assert_eq!(name, "another file name with spaces.zip");
    assert_eq!(size, 987654);

    let line3 = "not a yenc header size=123";
    assert!(parse_ybegin(line3).is_none());
}

#[tokio::test]
async fn test_nntp_resolve_metadata_mock() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let port = addr.port();

    tokio::spawn(async move {
        if let Ok((mut stream, _)) = listener.accept().await {
            let _ = stream
                .write_all(b"200 Welcome to mock NNTP server\r\n")
                .await;

            let mut reader = tokio::io::BufReader::new(&mut stream);
            let mut line = String::new();
            if reader.read_line(&mut line).await.is_ok() && line.starts_with("BODY") {
                let _ = stream.write_all(b"222 Body follows\r\n").await;
                let _ = stream
                    .write_all(b"=ybegin line=128 size=500 name=mock_file.txt\r\n")
                    .await;
                let _ = stream.write_all(b"yEnc data\r\n").await;
                let _ = stream.write_all(b".\r\n").await;
            }
        }
    });

    let worker = NntpWorker::new(
        crate::worker::WorkerBuilder::new(format!(
            "nntp://127.0.0.1:{}/msg-12345@domain.com",
            port
        ))
        .options,
    );

    let metadata = worker.resolve_metadata().await.unwrap();
    assert_eq!(metadata.name.unwrap(), "mock_file.txt");
    assert_eq!(metadata.total_length.unwrap(), 500);
}
