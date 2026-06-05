use crate::theme::Theme;
use ratatui::widgets::TableState;
use serde_json::json;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewState {
    Dashboard,
    MissionControl(String), // GID of the task
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct DownloadInfo {
    pub gid: String,
    pub status: String,
    #[serde(rename = "totalLength")]
    pub total_length: String,
    #[serde(rename = "completedLength")]
    pub completed_length: String,
    pub name: String,
}

pub struct App {
    pub client: reqwest::Client,
    pub downloads: Vec<DownloadInfo>,
    pub table_state: TableState,
    pub view_state: ViewState,
    pub should_quit: bool,
    pub error_msg: Option<String>,
    pub theme: Theme,
}

impl App {
    pub fn new() -> App {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        App {
            client: reqwest::Client::new(),
            downloads: Vec::new(),
            table_state,
            view_state: ViewState::Dashboard,
            should_quit: false,
            error_msg: None,
            theme: Theme::default(),
        }
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
                    self.downloads = serde_json::from_value(result.clone())?;
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
