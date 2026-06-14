use super::state::{App, DownloadInfo, FileInfo, ViewState};
use crate::theme::Theme;
use serde_json::{json, Value};
use std::collections::VecDeque;

impl App {
    pub(crate) async fn call_rpc(
        &self,
        method: &str,
        params: Option<Value>,
        id: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let mut body = json!({
            "jsonrpc": aura_core::JSONRPC_VERSION,
            "method": method,
            "id": id
        });
        if let Some(p) = params {
            body["params"] = p;
        }

        let mut request = self.client.post(&self.rpc_url).json(&body);
        if let Some(ref secret) = self.rpc_secret {
            request = request.header(aura_core::RPC_AUTH_HEADER, secret);
        }

        let res = request.send().await?;
        Ok(res.json().await?)
    }

    pub async fn fetch_files(&mut self, gid: &str) -> anyhow::Result<()> {
        let body = self
            .call_rpc("aura.getFiles", Some(json!([gid])), "tui-files")
            .await?;

        if let Some(result) = body.get("result") {
            let mut files: Vec<FileInfo> = serde_json::from_value(result.clone())?;

            if let Some(dl) = self.data.downloads.iter().find(|d| d.gid == gid) {
                if let Some(ref selection) = dl.selected_files {
                    for (i, f) in files.iter_mut().enumerate() {
                        f.selected = selection.get(i).copied().unwrap_or(true);
                    }
                } else {
                    for f in &mut files {
                        f.selected = true;
                    }
                }
            }

            self.data.files = files;
            self.ui.file_table_state.select(Some(0));
        }
        Ok(())
    }

    pub async fn submit_file_selection(&self, gid: &str) -> anyhow::Result<()> {
        let selection: Vec<bool> = self.data.files.iter().map(|f| f.selected).collect();
        let _ = self
            .call_rpc(
                "aura.setFileSelection",
                Some(json!([gid, selection])),
                "tui-set-files",
            )
            .await?;
        Ok(())
    }

    pub async fn fetch_config(&mut self) -> anyhow::Result<()> {
        let res = self.call_rpc("aura.getConfig", None, "tui-config").await;

        if let Ok(body) = res {
            if let Some(result) = body.get("result") {
                self.ui.theme = Theme::from_config(result);

                // Update tick rate from config if available
                if let Some(tui_cfg) = result.get("tui") {
                    if let Some(rate) = tui_cfg.get("tick_rate_ms").and_then(|v| v.as_u64()) {
                        self.tick_rate = std::time::Duration::from_millis(rate);
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        // OS Clipboard monitoring
        if let Some(cb) = &mut self.clipboard {
            if let Ok(text) = cb.get_text() {
                let trimmed = text.trim();
                // Check if clipboard content changed and looks like a valid URL, Magnet, or file path
                if trimmed != self.last_clipboard_content && !trimmed.is_empty() {
                    self.last_clipboard_content = trimmed.to_string();
                    let is_uri = trimmed.starts_with("http://")
                        || trimmed.starts_with("https://")
                        || trimmed.starts_with("ftp://")
                        || trimmed.starts_with("ftps://")
                        || trimmed.starts_with("magnet:?");

                    let is_valid = is_uri || std::path::Path::new(trimmed).exists();

                    if is_valid && self.ui.view_state == ViewState::Dashboard {
                        self.ui.discovery_input = trimmed.to_string();
                        self.ui.view_state = ViewState::Discovery;
                    }
                }
            }
        }

        let res = self.call_rpc("aura.tellActive", None, "tui").await;

        match res {
            Ok(body) => {
                if let Some(result) = body.get("result") {
                    let new_downloads: Vec<DownloadInfo> = serde_json::from_value(result.clone())?;
                    for dl in &new_downloads {
                        let speed = dl.download_speed.parse::<u64>().unwrap_or(0);
                        let history = self
                            .data
                            .speed_history
                            .entry(dl.gid.clone())
                            .or_insert_with(|| VecDeque::with_capacity(100));
                        if history.len() == 100 {
                            history.pop_front();
                        }
                        history.push_back(speed);
                    }
                    self.data.downloads = new_downloads;
                    self.ui.error_msg = None;
                }
            }
            Err(e) => {
                self.ui.error_msg = Some(format!("Daemon Connection Error: {}", e));
            }
        }

        // Fetch watch folder telemetry
        let stat_res = self.call_rpc("aura.getGlobalStat", None, "tui-stats").await;
        if let Ok(body) = stat_res {
            if let Some(result) = body.get("result") {
                if let Some(active) = result.get("watchFolderActive").and_then(|v| v.as_bool()) {
                    self.data.watch_folder_active = active;
                }
                if let Some(last) = result.get("lastIngestedFile").and_then(|v| v.as_str()) {
                    self.data.last_ingested_file = last.to_string();
                }
            }
        }

        Ok(())
    }

    pub async fn pause_selected(&mut self) -> anyhow::Result<()> {
        if let Some(i) = self.ui.table_state.selected() {
            if let Some(dl) = self.data.downloads.get(i) {
                let _ = self
                    .call_rpc("aura.pause", Some(json!([dl.gid])), "tui")
                    .await;
            }
        }
        Ok(())
    }

    pub async fn resume_selected(&mut self) -> anyhow::Result<()> {
        if let Some(i) = self.ui.table_state.selected() {
            if let Some(dl) = self.data.downloads.get(i) {
                let _ = self
                    .call_rpc("aura.unpause", Some(json!([dl.gid])), "tui")
                    .await;
            }
        }
        Ok(())
    }

    pub async fn submit_discovery(&mut self) -> anyhow::Result<()> {
        let input = self.ui.discovery_input.trim().to_string();
        if input.is_empty() {
            self.ui.view_state = ViewState::Dashboard;
            return Ok(());
        }

        let is_path = std::path::Path::new(&input).exists();

        if is_path {
            let is_dir = tokio::fs::metadata(&input)
                .await
                .map(|m| m.is_dir())
                .unwrap_or(false);
            if is_dir {
                self.call_rpc(
                    "aura.addFromFolder",
                    Some(json!([input, self.ui.discovery_recursive])),
                    "tui-discovery",
                )
                .await?;
            } else {
                self.call_rpc("aura.addFromFile", Some(json!([input])), "tui-discovery")
                    .await?;
            }
        } else {
            self.call_rpc("aura.addUri", Some(json!([[input]])), "tui-discovery")
                .await?;
        }

        self.ui.discovery_input.clear();
        self.ui.view_state = ViewState::Dashboard;
        Ok(())
    }

    pub async fn submit_command(&mut self) -> anyhow::Result<()> {
        let cmd = self.ui.command_input.trim().to_lowercase();
        self.ui.command_input.clear();
        self.ui.view_state = ViewState::Dashboard;

        if cmd == "quit" || cmd == "q" {
            self.should_quit = true;
        } else if cmd == "pause-all" {
            self.pause_all().await?;
        } else if cmd == "resume-all" {
            self.resume_all().await?;
        } else if cmd == "help" {
            self.ui.view_state = ViewState::Help;
        } else if let Some(stripped) = cmd.strip_prefix("add ") {
            let uri = stripped.trim().to_string();
            if !uri.is_empty() {
                self.call_rpc("aura.addUri", Some(json!([[uri]])), "tui-cmd-add")
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn pause_all(&mut self) -> anyhow::Result<()> {
        let _ = self.call_rpc("aura.pauseAll", None, "tui-pause-all").await;
        Ok(())
    }

    pub async fn resume_all(&mut self) -> anyhow::Result<()> {
        let _ = self
            .call_rpc("aura.unpauseAll", None, "tui-resume-all")
            .await;
        Ok(())
    }
}
