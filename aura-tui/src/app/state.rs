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
    pub discovery_input: String,
    pub discovery_recursive: bool,
    pub search_query: String,
    pub command_input: String,
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
            discovery_input: String::new(),
            discovery_recursive: false,
            search_query: String::new(),
            command_input: String::new(),
        }
    }
}
