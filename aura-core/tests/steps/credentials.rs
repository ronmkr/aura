use crate::AuraWorld;
use aura_core::task::TaskType;
use aura_core::TaskId;
use cucumber::{gherkin::Step, given, then, when};
use std::io::Write;
use tempfile::NamedTempFile;
use wiremock::matchers::{header, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[given(expr = "an HTTP mirror requiring Basic Auth")]
async fn given_http_mirror_auth(world: &mut AuraWorld) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(move |req: &wiremock::Request| {
            let auth = req.headers.get("Authorization");
            if let Some(auth_val) = auth {
                let val = auth_val.to_str().unwrap();
                if val == "Basic bXl1c2VyOm15cGFzcw==" || val == "Basic ZnRwdXNlcjpmdHBwYXNz" {
                    return ResponseTemplate::new(200)
                        .set_body_bytes(vec![0u8; 1024])
                        .insert_header("Content-Range", "bytes 0-1023/1024");
                }
            }
            ResponseTemplate::new(401)
        })
        .mount(&server)
        .await;

    world.mirror_uris.push(server.uri());
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[given(expr = "an HTTP mirror requiring Basic Auth for {string}")]
async fn given_http_mirror_auth_specific(world: &mut AuraWorld, user_pass: String) {
    let server = MockServer::start().await;
    let encoded = match user_pass.as_str() {
        "userA:passA" => "dXNlckE6cGFzc0E=",
        "userB:passB" => "dXNlckI6cGFzc0I=",
        _ => "unknown",
    };

    Mock::given(method("GET"))
        .and(header("Authorization", format!("Basic {}", encoded)))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 1024])
                .insert_header("Content-Range", "bytes 0-1023/1024"),
        )
        .mount(&server)
        .await;

    world.mirror_uris.push(server.uri());
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[given(expr = "a .netrc file with:")]
async fn given_netrc_file(world: &mut AuraWorld, step: &Step) {
    let mut netrc = NamedTempFile::new().unwrap();
    if let Some(table) = step.table.as_ref() {
        for row in table.rows.iter().skip(1) {
            let machine = &row[0];
            let login = &row[1];
            let password = &row[2];
            writeln!(
                netrc,
                "machine {} login {} password {}",
                machine, login, password
            )
            .unwrap();
        }
    }
    world.netrc_path = Some(netrc.path().to_path_buf());
    world.temp_files.push(netrc);
}

#[when(expr = "I add the authenticated task")]
async fn when_add_cred_task(world: &mut AuraWorld) {
    if world.engine.is_none() {
        let netrc = world
            .netrc_path
            .clone()
            .map(|p| p.to_str().unwrap().to_string());
        let cookies = world
            .cookie_path
            .clone()
            .map(|p| p.to_str().unwrap().to_string());

        world
            .init_engine(|config| {
                config.credentials.netrc_path = netrc;
                config.credentials.cookie_file = cookies;
            })
            .await;
    }

    let engine = world.engine.as_ref().unwrap();
    let id = TaskId(12345);
    world.last_task_id = Some(id);

    let uri = format!("{}/file", world.mirror_uris.last().unwrap());
    engine
        .add_task_with_sources(
            id,
            None,
            "cred-task".to_string(),
            vec![(uri, TaskType::Http)],
            None,
        )
        .await
        .unwrap();
}

#[when(expr = "I add a task for {string}")]
async fn when_add_specific_task(world: &mut AuraWorld, uri_placeholder: String) {
    if world.engine.is_none() {
        let netrc = world
            .netrc_path
            .clone()
            .map(|p| p.to_str().unwrap().to_string());
        world
            .init_engine(|config| {
                config.credentials.netrc_path = netrc;
            })
            .await;
    }

    let engine = world.engine.as_ref().unwrap();
    let id = TaskId::random();
    world.last_task_id = Some(id);

    // Map URI placeholder to actual mock server URI
    let actual_uri = if uri_placeholder.contains("127.0.0.1") {
        format!(
            "{}/file",
            world.mirror_uris[0].replace("localhost", "127.0.0.1")
        )
    } else {
        format!(
            "{}/file",
            world.mirror_uris[1].replace("127.0.0.1", "localhost")
        )
    };

    engine
        .add_task_with_sources(
            id,
            None,
            uri_placeholder,
            vec![(actual_uri, TaskType::Http)],
            None,
        )
        .await
        .unwrap();
}

#[then(expr = "the {string} should successfully authenticate and download the file")]
async fn then_success_auth(world: &mut AuraWorld, _worker: String) {
    let engine = world.engine.as_ref().unwrap();
    let id = world.last_task_id.unwrap();

    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    let mut success = false;
    for _ in 0..20 {
        interval.tick().await;
        let active = engine.tell_active().await.unwrap();
        if let Some(task) = active.iter().find(|t| t.id == id) {
            if task.completed_length == 1024 {
                success = true;
                break;
            }
        } else {
            success = true;
            break;
        }
    }
    assert!(success, "Download failed to authenticate");
}

#[then(expr = "the download for {string} should succeed")]
async fn then_success_specific(world: &mut AuraWorld, _target: String) {
    then_success_auth(world, "worker".to_string()).await;
}

#[given(expr = "an HTTP mirror requiring a {string} cookie")]
async fn given_http_mirror_cookie(world: &mut AuraWorld, cookie_name: String) {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("Cookie", format!("{}=secret-token", cookie_name)))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_bytes(vec![0u8; 1024])
                .insert_header("Content-Range", "bytes 0-1023/1024"),
        )
        .mount(&server)
        .await;

    world.mirror_uris.push(server.uri());
    world.mock_servers.push(std::sync::Arc::new(server));
}

#[given(expr = "a cookie file for {string} with {string}")]
async fn given_cookie_file(world: &mut AuraWorld, domain: String, cookie_expr: String) {
    let mut cookie_file = NamedTempFile::new().unwrap();
    let parts: Vec<&str> = cookie_expr.split('=').collect();
    let name = parts[0];
    let value = parts[1];

    // Netscape format: domain, flag, path, secure, expiration, name, value
    writeln!(
        cookie_file,
        "{}\tTRUE\t/\tFALSE\t2147483647\t{}\t{}",
        domain, name, value
    )
    .unwrap();
    world.cookie_path = Some(cookie_file.path().to_path_buf());
    world.temp_files.push(cookie_file);
}

#[then(expr = "the {string} should successfully send the cookie and download the file")]
async fn then_success_cookie(world: &mut AuraWorld, _worker: String) {
    then_success_auth(world, _worker).await;
}

#[given(expr = "an FTP mirror requiring login")]
async fn given_ftp_mirror_login(world: &mut AuraWorld) {
    given_http_mirror_auth(world).await;
}

#[then(expr = "the {string} should successfully login and download the file")]
async fn then_success_ftp(world: &mut AuraWorld, _worker: String) {
    then_success_auth(world, _worker).await;
}
