use crate::theme::Theme;
use ratatui::widgets::TableState;
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewState {
    Dashboard,
    MissionControl(String), // GID of the task
    FileSelector(String),   // GID of the task
    Discovery,
    Search,
    Help,
    CommandPalette,
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
    #[serde(rename = "recheckProgress", default)]
    pub recheck_progress: Option<String>,
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct FileInfo {
    pub length: u64,
    pub path: Vec<String>,
    #[serde(default)]
    pub selected: bool,
}

pub struct AppData {
    pub downloads: Vec<DownloadInfo>,
    pub files: Vec<FileInfo>,
    pub speed_history: HashMap<String, VecDeque<u64>>,
}

pub struct UiState {
    pub table_state: TableState,
    pub file_table_state: TableState,
    pub view_state: ViewState,
    pub error_msg: Option<String>,
    pub discovery_input: String,
    pub discovery_recursive: bool,
    pub search_query: String,
    pub command_input: String,
    pub theme: Theme,
}

pub struct App {
    pub client: reqwest::Client,
    pub data: AppData,
    pub ui: UiState,
    pub should_quit: bool,
    pub rpc_url: String,
    pub rpc_secret: Option<String>,
    pub tick_rate: std::time::Duration,
    pub clipboard: Option<arboard::Clipboard>,
    pub last_clipboard_content: String,
}

impl App {
    pub fn new(rpc_url: String, rpc_secret: Option<String>) -> App {
        let mut table_state = TableState::default();
        table_state.select(Some(0));
        let mut file_table_state = TableState::default();
        file_table_state.select(Some(0));

        let clipboard = arboard::Clipboard::new().ok();

        App {
            client: reqwest::Client::new(),
            data: AppData {
                downloads: Vec::new(),
                files: Vec::new(),
                speed_history: HashMap::new(),
            },
            ui: UiState {
                table_state,
                file_table_state,
                view_state: ViewState::Dashboard,
                error_msg: None,
                discovery_input: String::new(),
                discovery_recursive: false,
                search_query: String::new(),
                command_input: String::new(),
                theme: Theme::default(),
            },
            should_quit: false,
            rpc_url,
            rpc_secret,
            tick_rate: std::time::Duration::from_millis(500),
            clipboard,
            last_clipboard_content: String::new(),
        }
    }
}
