use serde_json::json;
use std::path::PathBuf;

pub async fn run_refresh(
    port: u16,
    secret: Option<String>,
    gid: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let secret = resolve_rpc_secret(secret);

    let url = format!("http://localhost:{}/jsonrpc", port);
    let mut req = client.post(&url);
    if let Some(ref sec) = secret {
        req = req.header("X-Aura-Token", sec);
    }

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "aura.refreshUri",
        "params": [gid.to_string()],
        "id": "cli-refresh"
    });

    let resp = req.json(&payload).send().await?;
    let body: serde_json::Value = resp.json().await?;

    if let Some(err) = body.get("error") {
        eprintln!("Error refreshing task: {}", err);
        std::process::exit(1);
    } else {
        println!("Refresh request sent successfully for GID {}", gid);
    }

    Ok(())
}

pub async fn run_show_files(
    port: u16,
    secret: Option<String>,
    gid: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let secret = resolve_rpc_secret(secret);

    let url = format!("http://localhost:{}/jsonrpc", port);
    let mut req = client.post(&url);
    if let Some(ref sec) = secret {
        req = req.header("X-Aura-Token", sec);
    }

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "aura.getFiles",
        "params": [gid.to_string()],
        "id": "cli-show-files"
    });

    let resp = req.json(&payload).send().await?;
    let body: serde_json::Value = resp.json().await?;

    if let Some(err) = body.get("error") {
        eprintln!("Error fetching files: {}", err);
        std::process::exit(1);
    }

    if let Some(result) = body.get("result") {
        let files: Vec<serde_json::Value> = serde_json::from_value(result.clone())?;
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
    let client = reqwest::Client::new();
    let secret = resolve_rpc_secret(secret);

    let url = format!("http://localhost:{}/jsonrpc", port);

    // First, get the files to know the total count
    let mut req = client.post(&url);
    if let Some(ref sec) = secret {
        req = req.header("X-Aura-Token", sec);
    }
    let get_payload = json!({
        "jsonrpc": "2.0",
        "method": "aura.getFiles",
        "params": [gid.to_string()],
        "id": "cli-select-pre-fetch"
    });
    let resp = req.json(&get_payload).send().await?;
    let body: serde_json::Value = resp.json().await?;

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
    let mut req = client.post(&url);
    if let Some(ref sec) = secret {
        req = req.header("X-Aura-Token", sec);
    }
    let set_payload = json!({
        "jsonrpc": "2.0",
        "method": "aura.setFileSelection",
        "params": [gid.to_string(), selection],
        "id": "cli-set-files"
    });

    let resp = req.json(&set_payload).send().await?;
    let body: serde_json::Value = resp.json().await?;

    if let Some(err) = body.get("error") {
        eprintln!("Error setting file selection: {}", err);
        std::process::exit(1);
    } else {
        println!("File selection updated successfully for GID {}", gid);
    }

    Ok(())
}

pub async fn run_add_from_folder(
    port: u16,
    secret: Option<String>,
    dir: &str,
    recursive: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let secret = resolve_rpc_secret(secret);

    let url = format!("http://localhost:{}/jsonrpc", port);
    let mut req = client.post(&url);
    if let Some(ref sec) = secret {
        req = req.header("X-Aura-Token", sec);
    }

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "aura.addFromFolder",
        "params": [dir, recursive],
        "id": "cli-add-folder"
    });

    let resp = req.json(&payload).send().await?;
    let body: serde_json::Value = resp.json().await?;

    if let Some(err) = body.get("error") {
        eprintln!("Error adding from folder: {}", err);
        std::process::exit(1);
    } else if let Some(result) = body.get("result") {
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
    let client = reqwest::Client::new();
    let secret = resolve_rpc_secret(secret);

    let url = format!("http://localhost:{}/jsonrpc", port);
    let mut req = client.post(&url);
    if let Some(ref sec) = secret {
        req = req.header("X-Aura-Token", sec);
    }

    let payload = json!({
        "jsonrpc": "2.0",
        "method": "aura.addFromFile",
        "params": [path],
        "id": "cli-add-file"
    });

    let resp = req.json(&payload).send().await?;
    let body: serde_json::Value = resp.json().await?;

    if let Some(err) = body.get("error") {
        eprintln!("Error adding from file: {}", err);
        std::process::exit(1);
    } else if let Some(result) = body.get("result") {
        let ids: Vec<String> = serde_json::from_value(result.clone())?;
        println!("Successfully added {} tasks from file.", ids.len());
        for id in ids {
            println!("  Added task GID: {}", id);
        }
    }

    Ok(())
}

fn resolve_rpc_secret(secret: Option<String>) -> Option<String> {
    match secret {
        Some(s) => Some(s),
        None => {
            let home = std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from);
            if let Some(h) = home {
                let p = h.join(".aura").join("rpc_secret");
                if p.exists() {
                    std::fs::read_to_string(&p)
                        .ok()
                        .map(|s| s.trim().to_string())
                } else {
                    None
                }
            } else {
                None
            }
        }
    }
}
