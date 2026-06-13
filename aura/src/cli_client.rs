use serde_json::{json, Value};

struct RpcClient {
    client: reqwest::Client,
    url: String,
    secret: Option<String>,
}

impl RpcClient {
    fn new(port: u16, secret: Option<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            url: format!("http://localhost:{}/jsonrpc", port),
            secret: aura_core::Config::resolve_rpc_secret(secret),
        }
    }

    async fn call(
        &self,
        method: &str,
        params: Vec<Value>,
        id: &str,
    ) -> Result<Value, Box<dyn std::error::Error>> {
        let mut req = self.client.post(&self.url);
        if let Some(ref sec) = self.secret {
            req = req.header(aura_core::RPC_AUTH_HEADER, sec);
        }

        let payload = json!({
            "jsonrpc": aura_core::JSONRPC_VERSION,
            "method": method,
            "params": params,
            "id": id
        });

        let resp = req.json(&payload).send().await?;
        let body: Value = resp.json().await?;
        Ok(body)
    }

    fn check_error(&self, body: &Value, action: &str) {
        if let Some(err) = body.get("error") {
            eprintln!("Error {}: {}", action, err);
            std::process::exit(1);
        }
    }
}

pub async fn run_refresh(
    port: u16,
    secret: Option<String>,
    gid: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(port, secret);
    let body = rpc
        .call(
            "aura.refreshUri",
            vec![json!(gid.to_string())],
            "cli-refresh",
        )
        .await?;

    rpc.check_error(&body, "refreshing task");
    println!("Refresh request sent successfully for GID {}", gid);
    Ok(())
}

pub async fn run_show_files(
    port: u16,
    secret: Option<String>,
    gid: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(port, secret);
    let body = rpc
        .call(
            "aura.getFiles",
            vec![json!(gid.to_string())],
            "cli-show-files",
        )
        .await?;

    rpc.check_error(&body, "fetching files");

    if let Some(result) = body.get("result") {
        let files: Vec<Value> = serde_json::from_value(result.clone())?;
        println!("{:<5} {:<10} {:<10}", "Idx", "Size", "Path");
        println!("{}", "-".repeat(40));
        for (i, f) in files.iter().enumerate() {
            let path = f
                .get("path")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("/")
                })
                .unwrap_or_default();
            let length = f.get("length").and_then(|v| v.as_u64()).unwrap_or(0);
            println!("{:<5} {:<10} {}", i, bytesize::ByteSize::b(length), path);
        }
    }

    Ok(())
}

pub async fn run_select_files(
    port: u16,
    secret: Option<String>,
    gid: u64,
    indices: Vec<usize>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(port, secret);

    // First, get the files to know the total count
    let body = rpc
        .call(
            "aura.getFiles",
            vec![json!(gid.to_string())],
            "cli-select-pre-fetch",
        )
        .await?;

    rpc.check_error(&body, "fetching files for selection");

    let total_files = body
        .get("result")
        .and_then(|r| r.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    if total_files == 0 {
        eprintln!(
            "No files found for GID {} or task is not a BitTorrent task.",
            gid
        );
        std::process::exit(1);
    }

    let mut selection = vec![false; total_files];
    for idx in indices {
        if idx < total_files {
            selection[idx] = true;
        } else {
            eprintln!(
                "Warning: Index {} is out of bounds (total files: {})",
                idx, total_files
            );
        }
    }

    // Now send the selection
    let body = rpc
        .call(
            "aura.setFileSelection",
            vec![json!(gid.to_string()), json!(selection)],
            "cli-set-files",
        )
        .await?;

    rpc.check_error(&body, "setting file selection");
    println!("File selection updated successfully for GID {}", gid);

    Ok(())
}

pub async fn run_add_from_folder(
    port: u16,
    secret: Option<String>,
    dir: &str,
    recursive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(port, secret);
    let body = rpc
        .call(
            "aura.addFromFolder",
            vec![json!(dir), json!(recursive)],
            "cli-add-folder",
        )
        .await?;

    rpc.check_error(&body, "adding from folder");
    if let Some(result) = body.get("result") {
        let ids: Vec<String> = serde_json::from_value(result.clone())?;
        println!("Successfully added {} tasks from folder.", ids.len());
        for id in ids {
            println!("  Added task GID: {}", id);
        }
    }

    Ok(())
}

pub async fn run_add_from_file(
    port: u16,
    secret: Option<String>,
    path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(port, secret);
    let body = rpc
        .call("aura.addFromFile", vec![json!(path)], "cli-add-file")
        .await?;

    rpc.check_error(&body, "adding from file");
    if let Some(result) = body.get("result") {
        let ids: Vec<String> = serde_json::from_value(result.clone())?;
        println!("Successfully added {} tasks from file.", ids.len());
        for id in ids {
            println!("  Added task GID: {}", id);
        }
    }

    Ok(())
}

pub async fn run_status(
    port: u16,
    secret: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(port, secret);
    let body = rpc.call("aura.getConfig", vec![], "cli-status").await?;

    rpc.check_error(&body, "fetching status");

    if let Some(config) = body.get("result") {
        println!("--- Aura Engine Status ---");

        let version = config
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        println!("{:<20}: {}", "Version", version);

        if let Some(bandwidth) = config.get("bandwidth") {
            let dl = bandwidth
                .get("global_download_limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let ul = bandwidth
                .get("global_upload_limit")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            println!("{:<20}: {}/s", "Global Download", bytesize::ByteSize::b(dl));
            println!("{:<20}: {}/s", "Global Upload", bytesize::ByteSize::b(ul));
        }

        if let Some(sched) = config.get("active_schedule") {
            if !sched.is_null() {
                let from = sched
                    .get("from")
                    .and_then(|v| v.as_str())
                    .unwrap_or("??:??");
                let to = sched.get("to").and_then(|v| v.as_str()).unwrap_or("??:??");
                let dl = sched
                    .get("download_limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                println!("{:<20}: Active ({} - {})", "Bandwidth Schedule", from, to);
                println!(
                    "{:<20}: {}/s (Scheduled)",
                    "Effective Limit",
                    bytesize::ByteSize::b(dl)
                );
            } else {
                println!("{:<20}: None", "Bandwidth Schedule");
            }
        }

        if let Some(next) = config.get("next_transition").and_then(|v| v.as_str()) {
            println!("{:<20}: {}", "Next Transition", next);
        }

        if let Some(limits) = config.get("limits") {
            let active = limits
                .get("max_active_tasks")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            println!("{:<20}: {}", "Max Active Tasks", active);
        }
    }

    Ok(())
}

pub async fn run_recheck(
    port: u16,
    secret: Option<String>,
    gid: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let rpc = RpcClient::new(port, secret);
    let body = rpc
        .call(
            "aura.forceRecheck",
            vec![json!(gid.to_string())],
            "cli-recheck",
        )
        .await?;

    rpc.check_error(&body, "forcing recheck");
    println!("Recheck request sent successfully for GID {}", gid);
    Ok(())
}
