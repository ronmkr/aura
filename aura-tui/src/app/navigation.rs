use super::state::{App, DownloadInfo};

impl App {
    pub fn filtered_downloads(&self) -> Vec<&DownloadInfo> {
        if self.ui.search_query.is_empty() {
            self.data.downloads.iter().collect()
        } else {
            let query = self.ui.search_query.to_lowercase();
            self.data
                .downloads
                .iter()
                .filter(|d| d.name.to_lowercase().contains(&query) || d.gid.contains(&query))
                .collect()
        }
    }

    pub fn next(&mut self) {
        let count = self.filtered_downloads().len();
        if count == 0 {
            return;
        }
        let i = match self.ui.table_state.selected() {
            Some(i) => {
                if i >= count.saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.ui.table_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        let count = self.filtered_downloads().len();
        if count == 0 {
            return;
        }
        let i = match self.ui.table_state.selected() {
            Some(i) => {
                if i == 0 {
                    count.saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.ui.table_state.select(Some(i));
    }

    pub fn first(&mut self) {
        if !self.filtered_downloads().is_empty() {
            self.ui.table_state.select(Some(0));
        }
    }

    pub fn clamp_selection(&mut self) {
        let count = self.filtered_downloads().len();
        if count == 0 {
            self.ui.table_state.select(None);
        } else if let Some(i) = self.ui.table_state.selected() {
            if i >= count {
                self.ui.table_state.select(Some(count - 1));
            }
        } else {
            self.ui.table_state.select(Some(0));
        }
    }

    pub fn last(&mut self) {
        let count = self.filtered_downloads().len();
        if count > 0 {
            self.ui.table_state.select(Some(count - 1));
        }
    }

    pub fn toggle_file_selection(&mut self) {
        if let Some(i) = self.ui.file_table_state.selected() {
            if let Some(f) = self.data.files.get_mut(i) {
                f.selected = !f.selected;
            }
        }
    }

    pub fn file_next(&mut self) {
        let i = match self.ui.file_table_state.selected() {
            Some(i) => {
                if i >= self.data.files.len().saturating_sub(1) {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.ui.file_table_state.select(Some(i));
    }

    pub fn file_previous(&mut self) {
        let i = match self.ui.file_table_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.data.files.len().saturating_sub(1)
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.ui.file_table_state.select(Some(i));
    }
}
