use crate::theme::Theme;
use ratatui::widgets::TableState;
use serde_json::json;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewState {
    Dashboard,
    MissionControl(String), // GID of the task
    FileSelector(String),   // GID of the task
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct DownloadInfo {
    pub gid: String,
    pub status: String,
    #[serde(rename = "totalLength")]
    pub total_length: String,
    #[serde(rename = "completedLength")]
    pub completed_length: String,
    #[serde(rename = "downloadSpeed", default)]
    pub download_speed: String,
    pub name: String,
    #[serde(rename = "selectedFiles", default)]
    pub selected_files: Option<Vec<bool>>,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct FileInfo {
    pub length: u64,
    pub path: Vec<String>,
    #[serde(default)]
    pub selected: bool,
}

pub struct App {
    pub client: reqwest::Client,
    pub downloads: Vec<DownloadInfo>,
    pub table_state: TableState,
    pub view_state: ViewState,
    pub should_quit: bool,
    pub error_msg: Option<String>,
    pub theme: Theme,
    pub speed_history: HashMap<String, VecDeque<u64>>,
    pub files: Vec<FileInfo>,
    pub file_table_state: TableState,
}

impl App {
    pub fn new() -> App {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        let mut file_table_state = TableState::default();
        file_table_state.select(Some(0));
        App {
            client: reqwest::Client::new(),
            downloads: Vec::new(),
            table_state,
            view_state: ViewState::Dashboard,
            should_quit: false,
            error_msg: None,
            theme: Theme::default(),
            speed_history: HashMap::new(),
            files: Vec::new(),
            file_table_state,
        }
    }

    pub async fn fetch_files(&mut self, gid: &str) -> anyhow::Result<()> {
        let res = self
            .client
            .post("http://localhost:6800/jsonrpc")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "aura.getFiles",
                "params": [gid],
                "id": "tui-files"
            }))
            .send()
            .await?;

        let body: serde_json::Value = res.json().await?;
        if let Some(result) = body.get("result") {
            let mut files: Vec<FileInfo> = serde_json::from_value(result.clone())?;

            if let Some(dl) = self.downloads.iter().find(|d| d.gid == gid) {
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

            self.files = files;
            self.file_table_state.select(Some(0));
        }
        Ok(())
    }

    pub async fn submit_file_selection(&self, gid: &str) -> anyhow::Result<()> {
        let selection: Vec<bool> = self.files.iter().map(|f| f.selected).collect();
        let _ = self
            .client
            .post("http://localhost:6800/jsonrpc")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "aura.setFileSelection",
                "params": [gid, selection],
                "id": "tui-set-files"
            }))
            .send()
            .await?;
        Ok(())
    }

    pub fn toggle_file_selection(&mut self) {
        if let Some(i) = self.file_table_state.selected() {
            if let Some(f) = self.files.get_mut(i) {
                f.selected = !f.selected;
            }
        }
    }

    pub fn file_next(&mut self) {
        let i = match self.file_table_state.selected() {
            Some(i) => {
                if i >= self.files.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.file_table_state.select(Some(i));
    }

    pub fn file_previous(&mut self) {
        let i = match self.file_table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.files.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.file_table_state.select(Some(i));
    }

    pub async fn fetch_theme(&mut self) -> anyhow::Result<()> {
        let res = self
            .client
            .post("http://localhost:6800/jsonrpc")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "aura.getConfig",
                "id": "tui-theme"
            }))
            .send()
            .await;

        if let Ok(response) = res {
            let body: serde_json::Value = response.json().await?;
            if let Some(result) = body.get("result") {
                self.theme = Theme::from_config(result);
            }
        }
        Ok(())
    }

    pub async fn tick(&mut self) -> anyhow::Result<()> {
        let res = self
            .client
            .post("http://localhost:6800/jsonrpc")
            .json(&json!({
                "jsonrpc": "2.0",
                "method": "aria2.tellActive",
                "id": "tui"
            }))
            .send()
            .await;

        match res {
            Ok(response) => {
                let body: serde_json::Value = response.json().await?;
                if let Some(result) = body.get("result") {
                    let new_downloads: Vec<DownloadInfo> = serde_json::from_value(result.clone())?;
                    for dl in &new_downloads {
                        let speed = dl.download_speed.parse::<u64>().unwrap_or(0);
                        let history = self
                            .speed_history
                            .entry(dl.gid.clone())
                            .or_insert_with(|| VecDeque::with_capacity(100));
                        if history.len() == 100 {
                            history.pop_front();
                        }
                        history.push_back(speed);
                    }
                    self.downloads = new_downloads;
                    self.error_msg = None;
                }
            }
            Err(e) => {
                self.error_msg = Some(format!("Daemon Connection Error: {}", e));
            }
        }
        Ok(())
    }

    pub fn next(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i >= self.downloads.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let i = match self.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.downloads.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.table_state.select(Some(i));
    }

    pub fn first(&mut self) {
        if !self.downloads.is_empty() {
            self.table_state.select(Some(0));
        }
    }

    pub fn last(&mut self) {
        if !self.downloads.is_empty() {
            self.table_state.select(Some(self.downloads.len() - 1));
        }
    }

    pub async fn pause_selected(&mut self) -> anyhow::Result<()> {
        if let Some(i) = self.table_state.selected() {
            if let Some(dl) = self.downloads.get(i) {
                let _ = self
                    .client
                    .post("http://localhost:6800/jsonrpc")
                    .json(&json!({
                        "jsonrpc": "2.0",
                        "method": "aria2.pause",
                        "params": [dl.gid],
                        "id": "tui"
                    }))
                    .send()
                    .await;
            }
        }
        Ok(())
    }

    pub async fn resume_selected(&mut self) -> anyhow::Result<()> {
        if let Some(i) = self.table_state.selected() {
            if let Some(dl) = self.downloads.get(i) {
                let _ = self
                    .client
                    .post("http://localhost:6800/jsonrpc")
                    .json(&json!({
                        "jsonrpc": "2.0",
                        "method": "aria2.unpause",
                        "params": [dl.gid],
                        "id": "tui"
                    }))
                    .send()
                    .await;
            }
        }
        Ok(())
    }
}
