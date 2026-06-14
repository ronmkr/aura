use crate::AuraWorld;
use aura_core::orchestrator::TaskQuerier;
use aura_core::task::{DownloadPhase, TaskType};
use cucumber::{given, then, when};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn parse_range(range_str: &str, total_len: usize) -> Option<(usize, usize)> {
    // S3 range header might be: bytes=0-0
    let clean_str = range_str.trim();
    if !clean_str.starts_with("bytes=") {
        return None;
    }
    let s = clean_str.strip_prefix("bytes=")?;
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        return None;
    }
    let start = parts[0].parse::<usize>().ok()?;
    let end = if parts[1].is_empty() {
        total_len - 1
    } else {
        parts[1].parse::<usize>().ok()?
    };
    if start <= end && start < total_len {
        Some((start, std::cmp::min(end, total_len - 1)))
    } else {
        None
    }
}

#[given(expr = "a mock S3 bucket {string} with key {string} containing {string}")]
async fn given_mock_s3_bucket(world: &mut AuraWorld, bucket: String, key: String, content: String) {
    let server = MockServer::start().await;
    let bytes = content.clone().into_bytes();
    let len = bytes.len() as u64;

    // Set AWS environment variables to point to this mock server
    std::env::set_var("AWS_ENDPOINT_URL", server.uri());
    std::env::set_var("AWS_ACCESS_KEY_ID", "mock_key");
    std::env::set_var("AWS_SECRET_ACCESS_KEY", "mock_secret");
    std::env::set_var("AWS_REGION", "us-east-1");

    // S3 HeadObject request mock
    Mock::given(method("HEAD"))
        .and(path(format!("/{}/{}", bucket, key)))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("Content-Length", len.to_string())
                .insert_header("ETag", "\"mock-etag\"")
                .insert_header("Last-Modified", "Thu, 11 Jun 2026 12:00:00 GMT"),
        )
        .mount(&server)
        .await;

    // S3 GetObject request mock
    Mock::given(method("GET"))
        .and(path(format!("/{}/{}", bucket, key)))
        .respond_with(move |req: &wiremock::Request| {
            let range_hdr = req
                .headers
                .get("Range")
                .or_else(|| req.headers.get("range"));
            if let Some(range_val) = range_hdr {
                if let Ok(range_str) = range_val.to_str() {
                    if let Some((start, end)) = parse_range(range_str, bytes.len()) {
                        let sub_bytes = bytes[start..=end].to_vec();
                        let sub_len = sub_bytes.len();
                        return ResponseTemplate::new(206)
                            .set_body_bytes(sub_bytes)
                            .insert_header(
                                "Content-Range",
                                format!("bytes {}-{}/{}", start, end, bytes.len()),
                            )
                            .insert_header("Content-Length", sub_len.to_string());
                    }
                }
            }
            ResponseTemplate::new(200)
                .set_body_bytes(bytes.clone())
                .insert_header("Content-Length", len.to_string())
        })
        .mount(&server)
        .await;

    world.mirror_uris.push(format!("s3://{}/{}", bucket, key));
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[given(expr = "a mock Google Drive file {string} containing {string}")]
async fn given_mock_gdrive_file(world: &mut AuraWorld, file_id: String, content: String) {
    let server = MockServer::start().await;
    let bytes = content.clone().into_bytes();
    let len = bytes.len() as u64;

    // Set GDrive endpoint override for testing
    std::env::set_var("AURA_GDRIVE_ENDPOINT", server.uri());

    // GDrive Metadata mock
    Mock::given(method("GET"))
        .and(path(format!("/drive/v3/files/{}", file_id)))
        .and(query_param("fields", "size,name"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "gdrive_file.bin",
            "size": len.to_string()
        })))
        .mount(&server)
        .await;

    // GDrive alt=media content mock
    Mock::given(method("GET"))
        .and(path(format!("/drive/v3/files/{}", file_id)))
        .and(query_param("alt", "media"))
        .respond_with(move |req: &wiremock::Request| {
            let range_hdr = req
                .headers
                .get("Range")
                .or_else(|| req.headers.get("range"));
            if let Some(range_val) = range_hdr {
                if let Ok(range_str) = range_val.to_str() {
                    if let Some((start, end)) = parse_range(range_str, bytes.len()) {
                        let sub_bytes = bytes[start..=end].to_vec();
                        let sub_len = sub_bytes.len();
                        return ResponseTemplate::new(206)
                            .set_body_bytes(sub_bytes)
                            .insert_header(
                                "Content-Range",
                                format!("bytes {}-{}/{}", start, end, bytes.len()),
                            )
                            .insert_header("Content-Length", sub_len.to_string());
                    }
                }
            }
            ResponseTemplate::new(200)
                .set_body_bytes(bytes.clone())
                .insert_header("Content-Length", len.to_string())
        })
        .mount(&server)
        .await;

    world.mirror_uris.push(format!("gdrive://{}", file_id));
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[given(expr = "a mock OneDrive item {string} containing {string}")]
async fn given_mock_onedrive_item(world: &mut AuraWorld, item_id: String, content: String) {
    let server = MockServer::start().await;
    let bytes = content.clone().into_bytes();
    let len = bytes.len() as u64;

    // Set OneDrive endpoint override for testing
    std::env::set_var("AURA_ONEDRIVE_ENDPOINT", server.uri());

    // OneDrive Metadata mock
    Mock::given(method("GET"))
        .and(path(format!("/v1.0/me/drive/items/{}", item_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "onedrive_file.bin",
            "size": len
        })))
        .mount(&server)
        .await;

    // OneDrive Content mock
    Mock::given(method("GET"))
        .and(path(format!("/v1.0/me/drive/items/{}/content", item_id)))
        .respond_with(move |req: &wiremock::Request| {
            let range_hdr = req
                .headers
                .get("Range")
                .or_else(|| req.headers.get("range"));
            if let Some(range_val) = range_hdr {
                if let Ok(range_str) = range_val.to_str() {
                    if let Some((start, end)) = parse_range(range_str, bytes.len()) {
                        let sub_bytes = bytes[start..=end].to_vec();
                        let sub_len = sub_bytes.len();
                        return ResponseTemplate::new(206)
                            .set_body_bytes(sub_bytes)
                            .insert_header(
                                "Content-Range",
                                format!("bytes {}-{}/{}", start, end, bytes.len()),
                            )
                            .insert_header("Content-Length", sub_len.to_string());
                    }
                }
            }
            ResponseTemplate::new(200)
                .set_body_bytes(bytes.clone())
                .insert_header("Content-Length", len.to_string())
        })
        .mount(&server)
        .await;

    world.mirror_uris.push(format!("onedrive://{}", item_id));
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[when(expr = "I add a task for S3 URL {string}")]
async fn when_add_s3_task(world: &mut AuraWorld, url: String) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
    let engine = world.engine.as_ref().unwrap();

    let handle = engine
        .add_task("s3_file.bin".to_string(), url, TaskType::S3)
        .await
        .expect("Failed to add S3 task");
    world.last_task_id = Some(handle.id());
}

#[when(expr = "I add a task for GDrive URL {string}")]
async fn when_add_gdrive_task(world: &mut AuraWorld, url: String) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
    let engine = world.engine.as_ref().unwrap();

    let handle = engine
        .add_task("gdrive_file.bin".to_string(), url, TaskType::GDrive)
        .await
        .expect("Failed to add GDrive task");
    world.last_task_id = Some(handle.id());
}

#[when(expr = "I add a task for OneDrive URL {string}")]
async fn when_add_onedrive_task(world: &mut AuraWorld, url: String) {
    if world.engine.is_none() {
        world.init_engine(|_| {}).await;
    }
    let engine = world.engine.as_ref().unwrap();

    let handle = engine
        .add_task("onedrive_file.bin".to_string(), url, TaskType::GDrive)
        .await
        .expect("Failed to add OneDrive task");
    world.last_task_id = Some(handle.id());
}

#[then(expr = "the download should complete successfully")]
async fn then_download_complete(world: &mut AuraWorld) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(50));
    let mut success = false;
    for _ in 0..100 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            if task.phase == DownloadPhase::Complete {
                success = true;
                break;
            }
            if task.phase == DownloadPhase::Error {
                panic!("Task failed during download: {:?}", task);
            }
        } else {
            // Check if it's already finished and cleared from active tasks list
            success = true;
            break;
        }
    }
    assert!(success, "Download task did not complete successfully");
}

#[then(expr = "the downloaded file should contain {string}")]
async fn then_file_contains_content(world: &mut AuraWorld, expected_content: String) {
    let filename = if world.mirror_uris.last().unwrap().starts_with("s3://") {
        "s3_file.bin"
    } else if world.mirror_uris.last().unwrap().starts_with("gdrive://") {
        "gdrive_file.bin"
    } else {
        "onedrive_file.bin"
    };

    let filepath = world.temp_dir.path().join(filename);

    // S3/GDrive tasks might take a split second to flush/complete complete state
    let mut content = Vec::new();
    for _ in 0..10 {
        if filepath.exists() {
            if let Ok(data) = tokio::fs::read(&filepath).await {
                content = data;
                if !content.is_empty() {
                    break;
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    let actual_str = String::from_utf8(content).expect("Invalid UTF-8 downloaded data");
    assert_eq!(actual_str, expected_content);
}
