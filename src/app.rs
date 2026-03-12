use crate::git::{self, DiffLine, FileEntry, FileStatus, GitRepo, Hunk, LinePair};
use color_eyre::Result;
use std::time::Instant;

pub struct App {
    pub repo: GitRepo,
    pub file_entries: Vec<FileEntry>,
    pub selected_index: usize,
    pub active_panel: Panel,
    pub diff_state: Option<DiffState>,
    pub branch_name: String,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub last_refresh: Instant,
}

pub struct DiffState {
    pub file_path: String,
    pub left_lines: Vec<DiffLine>,
    pub right_lines: Vec<DiffLine>,
    pub line_mapping: Vec<LinePair>,
    pub hunks: Vec<Hunk>,
    pub current_hunk: usize,
    pub scroll: usize,
    pub max_scroll: usize,
    // Keep raw content for hunk application
    pub old_content: String,
    pub new_content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    FileList,
    DiffView,
}

#[derive(Debug)]
pub enum Message {
    MoveUp,
    MoveDown,
    SelectFile,
    SwitchPanel,
    StageFile,
    StageHunk,
    UnstageFile,
    DiscardChanges,
    Refresh,
    AutoRefresh,
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
            should_quit: false,
            status_message: None,
            last_refresh: Instant::now(),
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
                    if ds.hunks.is_empty() {
                        ds.scroll = ds.scroll.saturating_sub(1);
                    } else if ds.current_hunk > 0 {
                        ds.current_hunk -= 1;
                        ds.scroll = ds.hunks[ds.current_hunk].display_start;
                    }
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
                    if ds.hunks.is_empty() {
                        if ds.scroll < ds.max_scroll {
                            ds.scroll += 1;
                        }
                    } else if ds.current_hunk < ds.hunks.len() - 1 {
                        ds.current_hunk += 1;
                        ds.scroll = ds.hunks[ds.current_hunk].display_start;
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
            Message::StageHunk => {
                self.stage_current_hunk()?;
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
                // Also reload current diff if viewing one
                if self.diff_state.is_some() {
                    self.load_selected_diff()?;
                }
            }
            Message::AutoRefresh => {
                self.refresh()?;
                self.last_refresh = Instant::now();
            }
        }
        Ok(())
    }

    fn stage_current_hunk(&mut self) -> Result<()> {
        let (path, old, new, hunk_idx, hunk_count) = {
            let Some(ds) = &self.diff_state else {
                self.status_message = Some("No diff to stage from".into());
                return Ok(());
            };
            if ds.hunks.is_empty() {
                self.status_message = Some("No hunks to stage".into());
                return Ok(());
            }
            (
                ds.file_path.clone(),
                ds.old_content.clone(),
                ds.new_content.clone(),
                ds.current_hunk,
                ds.hunks.len(),
            )
        };

        // Apply only this hunk to produce new index content
        let patched = git::apply_hunk(&old, &new, hunk_idx);
        self.repo.stage_content(&path, &patched)?;
        self.status_message = Some(format!(
            "Staged hunk {}/{} of {path}",
            hunk_idx + 1,
            hunk_count
        ));
        self.refresh()?;
        self.load_selected_diff()?;
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
                    let (left_lines, right_lines, line_mapping, hunks) =
                        git::compute_diff(&old, &new);
                    let max_scroll = left_lines.len().max(right_lines.len());
                    // Preserve current hunk if still valid, else reset to 0
                    let prev_hunk = self
                        .diff_state
                        .as_ref()
                        .filter(|ds| ds.file_path == path)
                        .map(|ds| ds.current_hunk.min(hunks.len().saturating_sub(1)))
                        .unwrap_or(0);
                    let scroll = hunks
                        .get(prev_hunk)
                        .map(|h| h.display_start)
                        .unwrap_or(0);
                    self.diff_state = Some(DiffState {
                        file_path: path,
                        left_lines,
                        right_lines,
                        line_mapping,
                        hunks,
                        current_hunk: prev_hunk,
                        scroll,
                        max_scroll,
                        old_content: old,
                        new_content: new,
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
