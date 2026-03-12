use crate::git::{self, DiffLine, FileEntry, FileStatus, GitRepo, LinePair};
use color_eyre::Result;

pub struct App {
    pub repo: GitRepo,
    pub file_entries: Vec<FileEntry>,
    pub selected_index: usize,
    pub active_panel: Panel,
    pub diff_state: Option<DiffState>,
    pub branch_name: String,
    pub mode: AppMode,
    pub should_quit: bool,
    pub status_message: Option<String>,
}

pub struct DiffState {
    pub file_path: String,
    pub left_lines: Vec<DiffLine>,
    pub right_lines: Vec<DiffLine>,
    pub line_mapping: Vec<LinePair>,
    pub scroll: usize,
    pub max_scroll: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    FileList,
    DiffView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    Normal,
}

#[derive(Debug)]
pub enum Message {
    MoveUp,
    MoveDown,
    SelectFile,
    SwitchPanel,
    ScrollUp,
    ScrollDown,
    StageFile,
    UnstageFile,
    DiscardChanges,
    Refresh,
    Quit,
}

impl App {
    pub fn new(path: &str) -> Result<Self> {
        let repo = GitRepo::open(path)?;
        let branch_name = repo.branch_name();
        let file_entries = repo.get_file_statuses()?;

        Ok(Self {
            repo,
            file_entries,
            selected_index: 0,
            active_panel: Panel::FileList,
            diff_state: None,
            branch_name,
            mode: AppMode::Normal,
            should_quit: false,
            status_message: None,
        })
    }

    pub fn update(&mut self, msg: Message) -> Result<()> {
        match msg {
            Message::Quit => {
                self.should_quit = true;
            }
            Message::MoveUp => {
                if self.active_panel == Panel::FileList {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                } else if let Some(ds) = &mut self.diff_state {
                    ds.scroll = ds.scroll.saturating_sub(1);
                }
            }
            Message::MoveDown => {
                if self.active_panel == Panel::FileList {
                    if !self.file_entries.is_empty()
                        && self.selected_index < self.file_entries.len() - 1
                    {
                        self.selected_index += 1;
                    }
                } else if let Some(ds) = &mut self.diff_state {
                    if ds.scroll < ds.max_scroll {
                        ds.scroll += 1;
                    }
                }
            }
            Message::ScrollUp => {
                if let Some(ds) = &mut self.diff_state {
                    ds.scroll = ds.scroll.saturating_sub(1);
                }
            }
            Message::ScrollDown => {
                if let Some(ds) = &mut self.diff_state {
                    if ds.scroll < ds.max_scroll {
                        ds.scroll += 1;
                    }
                }
            }
            Message::SelectFile => {
                self.load_selected_diff()?;
            }
            Message::SwitchPanel => {
                self.active_panel = match self.active_panel {
                    Panel::FileList => Panel::DiffView,
                    Panel::DiffView => Panel::FileList,
                };
            }
            Message::StageFile => {
                if let Some(entry) = self.file_entries.get(self.selected_index) {
                    let path = entry.path.clone();
                    self.repo.stage_file(&path)?;
                    self.status_message = Some(format!("Staged: {path}"));
                    self.refresh()?;
                    self.load_selected_diff()?;
                }
            }
            Message::UnstageFile => {
                if let Some(entry) = self.file_entries.get(self.selected_index) {
                    let path = entry.path.clone();
                    self.repo.unstage_file(&path)?;
                    self.status_message = Some(format!("Unstaged: {path}"));
                    self.refresh()?;
                    self.load_selected_diff()?;
                }
            }
            Message::DiscardChanges => {
                if let Some(entry) = self.file_entries.get(self.selected_index) {
                    let path = entry.path.clone();
                    match entry.status {
                        FileStatus::Unstaged(_) | FileStatus::Conflict => {
                            self.repo.discard_changes(&path)?;
                            self.status_message = Some(format!("Discarded: {path}"));
                            self.refresh()?;
                        }
                        _ => {
                            self.status_message =
                                Some("Can only discard unstaged changes".into());
                        }
                    }
                }
            }
            Message::Refresh => {
                self.refresh()?;
            }
        }
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        self.file_entries = self.repo.get_file_statuses()?;
        self.branch_name = self.repo.branch_name();
        if self.selected_index >= self.file_entries.len() {
            self.selected_index = self.file_entries.len().saturating_sub(1);
        }
        Ok(())
    }

    fn load_selected_diff(&mut self) -> Result<()> {
        if let Some(entry) = self.file_entries.get(self.selected_index) {
            let path = entry.path.clone();
            match self.repo.get_diff_content(&path) {
                Ok((old, new)) => {
                    let (left_lines, right_lines, line_mapping) =
                        git::compute_diff(&old, &new);
                    let max_scroll = left_lines.len().max(right_lines.len());
                    self.diff_state = Some(DiffState {
                        file_path: path,
                        left_lines,
                        right_lines,
                        line_mapping,
                        scroll: 0,
                        max_scroll,
                    });
                }
                Err(_) => {
                    self.diff_state = None;
                    self.status_message = Some(format!("Cannot diff: {path}"));
                }
            }
        }
        Ok(())
    }
}
