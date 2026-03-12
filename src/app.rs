use crate::git::{self, BranchEntry, DiffLine, FileEntry, FileStatus, GitRepo, Hunk, LinePair, LogEntry, StashEntry};
use crate::keymap::KeymapName;
use color_eyre::Result;
use std::collections::BTreeSet;
use std::time::Instant;

pub struct App {
    pub repo: GitRepo,
    pub file_entries: Vec<FileEntry>,
    pub selected_index: usize,
    pub active_panel: Panel,
    pub diff_state: Option<DiffState>,
    pub branch_name: String,
    pub ahead_behind: (usize, usize),
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub last_refresh: Instant,
    pub keymap: KeymapName,
    pub overlay: Overlay,
    /// Saved commit message from undo, pre-filled on next commit
    pub saved_commit_msg: Option<String>,
    /// File list filter (None = not filtering)
    pub file_filter: Option<String>,
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
    pub old_content: String,
    pub new_content: String,
    pub view_mode: DiffViewMode,
    pub cursor_line: usize,
    pub selected_lines: BTreeSet<usize>,
    pub hunk_changed_rows: Vec<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    FileList,
    DiffView,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffViewMode {
    HunkNav,
    LineNav,
}

// ── Overlay (popups) ─────────────────────────────────────────────────────────

pub enum Overlay {
    None,
    Confirm {
        message: String,
        action: PendingAction,
    },
    CommitInput {
        input: TextInput,
        amend: bool,
    },
    GitLog {
        entries: Vec<LogEntry>,
        selected: usize,
        scroll: usize,
    },
    StashList {
        entries: Vec<StashEntry>,
        selected: usize,
    },
    BranchList {
        entries: Vec<BranchEntry>,
        selected: usize,
        creating: Option<String>,
    },
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    CommitAmend,
    UndoLastCommit,
    DiscardChanges { path: String },
}

impl Overlay {
    pub fn is_active(&self) -> bool {
        !matches!(self, Overlay::None)
    }
}

// ── TextInput ────────────────────────────────────────────────────────────────

pub struct TextInput {
    pub lines: Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
}

impl TextInput {
    pub fn new(initial: &str) -> Self {
        let lines: Vec<String> = if initial.is_empty() {
            vec![String::new()]
        } else {
            initial.lines().map(String::from).collect()
        };
        let cursor_row = lines.len().saturating_sub(1);
        let cursor_col = lines.last().map(|l| l.len()).unwrap_or(0);
        Self {
            lines,
            cursor_row,
            cursor_col,
        }
    }

    pub fn insert_char(&mut self, ch: char) {
        let line = &mut self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        line.insert(byte_idx, ch);
        self.cursor_col += 1;
    }

    pub fn insert_newline(&mut self) {
        let line = &mut self.lines[self.cursor_row];
        let byte_idx = char_to_byte_idx(line, self.cursor_col);
        let rest = line[byte_idx..].to_string();
        line.truncate(byte_idx);
        self.cursor_row += 1;
        self.cursor_col = 0;
        self.lines.insert(self.cursor_row, rest);
    }

    pub fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_row];
            let byte_idx = char_to_byte_idx(line, self.cursor_col - 1);
            let end_idx = char_to_byte_idx(line, self.cursor_col);
            line.replace_range(byte_idx..end_idx, "");
            self.cursor_col -= 1;
        } else if self.cursor_row > 0 {
            let removed = self.lines.remove(self.cursor_row);
            self.cursor_row -= 1;
            self.cursor_col = self.lines[self.cursor_row].chars().count();
            self.lines[self.cursor_row].push_str(&removed);
        }
    }

    pub fn move_left(&mut self) {
        if self.cursor_col > 0 {
            self.cursor_col -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let len = self.lines[self.cursor_row].chars().count();
        if self.cursor_col < len {
            self.cursor_col += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor_row > 0 {
            self.cursor_row -= 1;
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
        }
    }

    pub fn move_down(&mut self) {
        if self.cursor_row < self.lines.len() - 1 {
            self.cursor_row += 1;
            let len = self.lines[self.cursor_row].chars().count();
            self.cursor_col = self.cursor_col.min(len);
        }
    }

    pub fn move_home(&mut self) {
        self.cursor_col = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor_col = self.lines[self.cursor_row].chars().count();
    }

    pub fn to_string(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.trim().is_empty())
    }
}

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

// ── Messages ─────────────────────────────────────────────────────────────────

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
    EnterLineMode,
    ExitLineMode,
    ToggleLine,
    StageLines,
    SelectAllLines,
    CycleKeymap,
    YankToClipboard,
    // Filter
    StartFilter,
    ClearFilter,
    // Stash
    StashSave,
    OpenStashList,
    StashPop,
    StashApply,
    StashDrop,
    // Branches
    OpenBranchList,
    CheckoutBranch,
    StartCreateBranch,
    ConfirmCreateBranch,
    // Commit / log
    OpenCommit,
    OpenCommitAmend,
    UndoLastCommit,
    OpenGitLog,
    // Overlay actions (handled by overlay key routing, not keymap)
    CloseOverlay,
    ConfirmCommit,
    ConfirmAction,
    Quit,
}

// ── App ──────────────────────────────────────────────────────────────────────

impl App {
    pub fn new(path: &str, keymap: KeymapName) -> Result<Self> {
        let repo = GitRepo::open(path)?;
        let branch_name = repo.branch_name();
        let ahead_behind = repo.ahead_behind();
        let file_entries = repo.get_file_statuses()?;

        Ok(Self {
            repo,
            file_entries,
            selected_index: 0,
            active_panel: Panel::FileList,
            diff_state: None,
            branch_name,
            ahead_behind,
            should_quit: false,
            status_message: None,
            last_refresh: Instant::now(),
            keymap,
            overlay: Overlay::None,
            saved_commit_msg: None,
            file_filter: None,
        })
    }

    pub fn update(&mut self, msg: Message) -> Result<()> {
        match msg {
            Message::Quit => {
                if self.overlay.is_active() {
                    self.overlay = Overlay::None;
                } else {
                    self.should_quit = true;
                }
            }
            Message::CloseOverlay => {
                self.overlay = Overlay::None;
            }
            Message::MoveUp => self.handle_move_up(),
            Message::MoveDown => self.handle_move_down(),
            Message::SelectFile => {
                self.load_selected_diff()?;
            }
            Message::SwitchPanel => {
                if self.active_panel == Panel::DiffView {
                    if let Some(ds) = &mut self.diff_state {
                        ds.view_mode = DiffViewMode::HunkNav;
                        ds.selected_lines.clear();
                    }
                }
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
                            self.overlay = Overlay::Confirm {
                                message: format!("Discard changes to {path}? This cannot be undone."),
                                action: PendingAction::DiscardChanges { path },
                            };
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
                if self.diff_state.is_some() {
                    self.load_selected_diff()?;
                }
            }
            Message::AutoRefresh => {
                self.refresh()?;
                self.last_refresh = Instant::now();
            }
            Message::EnterLineMode => {
                if let Some(ds) = &mut self.diff_state {
                    if ds.hunks.is_empty() {
                        return Ok(());
                    }
                    let hunk = &ds.hunks[ds.current_hunk];
                    let changed = git::changed_rows_in_hunk(hunk, &ds.left_lines);
                    if changed.is_empty() {
                        return Ok(());
                    }
                    ds.cursor_line = changed[0];
                    ds.scroll = ds.cursor_line.saturating_sub(3);
                    ds.hunk_changed_rows = changed;
                    ds.selected_lines.clear();
                    ds.view_mode = DiffViewMode::LineNav;
                    self.status_message = Some(match self.keymap {
                        KeymapName::Vim => "Line mode: j/k navigate, Space toggle, a all, s stage, Esc back".into(),
                        KeymapName::Helix => "Line mode: j/k navigate, x toggle, X all, s stage, Esc back".into(),
                    });
                }
            }
            Message::ExitLineMode => {
                if let Some(ds) = &mut self.diff_state {
                    ds.view_mode = DiffViewMode::HunkNav;
                    ds.selected_lines.clear();
                    ds.scroll = ds.hunks.get(ds.current_hunk)
                        .map(|h| h.display_start)
                        .unwrap_or(0);
                    self.status_message = None;
                }
            }
            Message::ToggleLine => {
                if let Some(ds) = &mut self.diff_state {
                    if ds.view_mode == DiffViewMode::LineNav {
                        let row = ds.cursor_line;
                        if ds.selected_lines.contains(&row) {
                            ds.selected_lines.remove(&row);
                        } else {
                            ds.selected_lines.insert(row);
                        }
                    }
                }
            }
            Message::SelectAllLines => {
                if let Some(ds) = &mut self.diff_state {
                    if ds.view_mode == DiffViewMode::LineNav {
                        if ds.selected_lines.len() == ds.hunk_changed_rows.len() {
                            ds.selected_lines.clear();
                        } else {
                            ds.selected_lines = ds.hunk_changed_rows.iter().copied().collect();
                        }
                    }
                }
            }
            Message::StageLines => {
                self.stage_selected_lines()?;
            }
            Message::CycleKeymap => {
                self.keymap = self.keymap.cycle();
                self.status_message = Some(format!("Keymap: {}", self.keymap.label()));
            }
            Message::YankToClipboard => {
                let text = self.get_yank_text();
                if let Some(text) = text {
                    match cli_clipboard::set_contents(text.clone()) {
                        Ok(()) => {
                            let preview: String = text.chars().take(40).collect();
                            self.status_message = Some(format!("Yanked: {preview}"));
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Clipboard error: {e}"));
                        }
                    }
                }
            }

            // ── Filter ────────────────────────────────────────────────────
            Message::StartFilter => {
                self.file_filter = Some(String::new());
            }
            Message::ClearFilter => {
                self.file_filter = None;
            }

            // ── Branches ──────────────────────────────────────────────────
            Message::OpenBranchList => {
                match self.repo.list_branches() {
                    Ok(entries) => {
                        let sel = entries.iter().position(|b| b.is_current).unwrap_or(0);
                        self.overlay = Overlay::BranchList {
                            entries,
                            selected: sel,
                            creating: None,
                        };
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Branch list failed: {e}"));
                    }
                }
            }
            Message::CheckoutBranch => {
                if let Overlay::BranchList { entries, selected, .. } = &self.overlay {
                    if let Some(entry) = entries.get(*selected) {
                        let name = if entry.is_remote {
                            // For remote branches like "origin/foo", checkout as local "foo"
                            entry.name.split('/').skip(1).collect::<Vec<_>>().join("/")
                        } else {
                            entry.name.clone()
                        };
                        self.overlay = Overlay::None;
                        match self.repo.checkout_branch(&name) {
                            Ok(()) => {
                                self.status_message = Some(format!("Switched to {name}"));
                                self.refresh()?;
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Checkout failed: {e}"));
                            }
                        }
                    }
                }
            }
            Message::StartCreateBranch => {
                if let Overlay::BranchList { ref mut creating, .. } = self.overlay {
                    *creating = Some(String::new());
                }
            }
            Message::ConfirmCreateBranch => {
                if let Overlay::BranchList { creating, .. } = &self.overlay {
                    if let Some(name) = creating {
                        let name = name.clone();
                        if name.is_empty() {
                            self.status_message = Some("Branch name cannot be empty".into());
                        } else {
                            self.overlay = Overlay::None;
                            match self.repo.create_branch(&name) {
                                Ok(()) => {
                                    self.status_message = Some(format!("Created and switched to {name}"));
                                    self.refresh()?;
                                }
                                Err(e) => {
                                    self.status_message = Some(format!("Create branch failed: {e}"));
                                }
                            }
                        }
                    }
                }
            }

            // ── Stash ─────────────────────────────────────────────────────
            Message::StashSave => {
                match self.repo.stash_save(None) {
                    Ok(()) => {
                        self.status_message = Some("Stashed changes".into());
                        self.refresh()?;
                        if self.diff_state.is_some() {
                            self.load_selected_diff()?;
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Stash failed: {e}"));
                    }
                }
            }
            Message::OpenStashList => {
                match self.repo.stash_list() {
                    Ok(entries) => {
                        if entries.is_empty() {
                            self.status_message = Some("No stashes".into());
                        } else {
                            self.overlay = Overlay::StashList {
                                entries,
                                selected: 0,
                            };
                        }
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Stash list failed: {e}"));
                    }
                }
            }
            Message::StashPop => {
                if let Overlay::StashList { entries, selected } = &self.overlay {
                    let idx = entries.get(*selected).map(|e| e.index);
                    if let Some(idx) = idx {
                        let sel = *selected;
                        self.overlay = Overlay::None;
                        match self.repo.stash_pop(idx) {
                            Ok(()) => {
                                self.status_message = Some(format!("Popped stash@{{{sel}}}"));
                                self.refresh()?;
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Stash pop failed: {e}"));
                            }
                        }
                    }
                }
            }
            Message::StashApply => {
                if let Overlay::StashList { entries, selected, .. } = &self.overlay {
                    let idx = entries.get(*selected).map(|e| e.index);
                    if let Some(idx) = idx {
                        let sel = *selected;
                        match self.repo.stash_apply(idx) {
                            Ok(()) => {
                                self.status_message = Some(format!("Applied stash@{{{sel}}}"));
                                self.refresh()?;
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Stash apply failed: {e}"));
                            }
                        }
                    }
                }
            }
            Message::StashDrop => {
                if let Overlay::StashList { entries, selected } = &self.overlay {
                    let idx = entries.get(*selected).map(|e| e.index);
                    let sel = *selected;
                    if let Some(idx) = idx {
                        self.overlay = Overlay::None;
                        match self.repo.stash_drop(idx) {
                            Ok(()) => {
                                self.status_message = Some(format!("Dropped stash@{{{sel}}}"));
                                // Reopen stash list
                                if let Ok(new_entries) = self.repo.stash_list() {
                                    if !new_entries.is_empty() {
                                        self.overlay = Overlay::StashList {
                                            selected: sel.min(new_entries.len().saturating_sub(1)),
                                            entries: new_entries,
                                        };
                                    }
                                }
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Stash drop failed: {e}"));
                            }
                        }
                    }
                }
            }

            // ── Commit / Log ─────────────────────────────────────────────
            Message::OpenCommit => {
                if !self.repo.has_staged_changes() {
                    self.status_message = Some("Nothing staged to commit".into());
                    return Ok(());
                }
                let initial = self.saved_commit_msg.take().unwrap_or_default();
                self.overlay = Overlay::CommitInput {
                    input: TextInput::new(&initial),
                    amend: false,
                };
            }
            Message::OpenCommitAmend => {
                self.overlay = Overlay::Confirm {
                    message: "Amend the last commit? This will rewrite history.".into(),
                    action: PendingAction::CommitAmend,
                };
            }
            Message::ConfirmCommit => {
                self.do_commit()?;
            }
            Message::ConfirmAction => {
                let action = match &self.overlay {
                    Overlay::Confirm { action, .. } => action.clone(),
                    _ => return Ok(()),
                };
                self.overlay = Overlay::None;
                match action {
                    PendingAction::CommitAmend => {
                        let initial = self.repo.last_commit_message().unwrap_or_default();
                        self.overlay = Overlay::CommitInput {
                            input: TextInput::new(&initial),
                            amend: true,
                        };
                    }
                    PendingAction::UndoLastCommit => {
                        match self.repo.undo_last_commit() {
                            Ok(msg) => {
                                self.status_message =
                                    Some("Undone last commit (message saved)".into());
                                self.saved_commit_msg = Some(msg);
                                self.refresh()?;
                                if self.diff_state.is_some() {
                                    self.load_selected_diff()?;
                                }
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Undo failed: {e}"));
                            }
                        }
                    }
                    PendingAction::DiscardChanges { path } => {
                        match self.repo.discard_changes(&path) {
                            Ok(()) => {
                                self.status_message = Some(format!("Discarded: {path}"));
                                self.refresh()?;
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Discard failed: {e}"));
                            }
                        }
                    }
                }
            }
            Message::UndoLastCommit => {
                self.overlay = Overlay::Confirm {
                    message: "Undo the last commit? Changes will be unstaged.".into(),
                    action: PendingAction::UndoLastCommit,
                };
            }
            Message::OpenGitLog => {
                match self.repo.get_log(100) {
                    Ok(entries) => {
                        self.overlay = Overlay::GitLog {
                            entries,
                            selected: 0,
                            scroll: 0,
                        };
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Log failed: {e}"));
                    }
                }
            }
        }
        Ok(())
    }

    fn do_commit(&mut self) -> Result<()> {
        let (message, amend) = match &self.overlay {
            Overlay::CommitInput { input, amend } => {
                if input.is_empty() {
                    self.status_message = Some("Commit message cannot be empty".into());
                    return Ok(());
                }
                (input.to_string(), *amend)
            }
            _ => return Ok(()),
        };

        let result = if amend {
            self.repo.commit_amend(&message)
        } else {
            self.repo.commit(&message)
        };

        match result {
            Ok(hash) => {
                let verb = if amend { "Amended" } else { "Committed" };
                self.status_message = Some(format!("{verb}: {hash}"));
                self.overlay = Overlay::None;
                self.saved_commit_msg = None;
                self.refresh()?;
                if self.diff_state.is_some() {
                    self.load_selected_diff()?;
                }
            }
            Err(e) => {
                self.status_message = Some(format!("Commit failed: {e}"));
            }
        }
        Ok(())
    }

    fn handle_move_up(&mut self) {
        match &mut self.overlay {
            Overlay::GitLog {
                selected, scroll, ..
            } => {
                if *selected > 0 {
                    *selected -= 1;
                    if *selected < *scroll {
                        *scroll = *selected;
                    }
                }
                return;
            }
            Overlay::CommitInput { input, .. } => {
                input.move_up();
                return;
            }
            Overlay::StashList { selected, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                }
                return;
            }
            Overlay::BranchList { selected, creating: None, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                }
                return;
            }
            Overlay::BranchList { .. } => return,
            Overlay::Confirm { .. } => return,
            Overlay::None => {}
        }

        if self.active_panel == Panel::FileList {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            }
            return;
        }
        let Some(ds) = &mut self.diff_state else { return };
        match ds.view_mode {
            DiffViewMode::HunkNav => {
                if ds.hunks.is_empty() {
                    ds.scroll = ds.scroll.saturating_sub(1);
                } else if ds.current_hunk > 0 {
                    ds.current_hunk -= 1;
                    ds.scroll = ds.hunks[ds.current_hunk].display_start;
                }
            }
            DiffViewMode::LineNav => {
                if let Some(pos) = ds.hunk_changed_rows.iter().position(|&r| r == ds.cursor_line) {
                    if pos > 0 {
                        ds.cursor_line = ds.hunk_changed_rows[pos - 1];
                        if ds.cursor_line < ds.scroll {
                            ds.scroll = ds.cursor_line;
                        }
                    }
                }
            }
        }
    }

    fn handle_move_down(&mut self) {
        match &mut self.overlay {
            Overlay::GitLog {
                entries,
                selected,
                scroll,
            } => {
                if *selected < entries.len().saturating_sub(1) {
                    *selected += 1;
                    if *selected >= *scroll + 20 {
                        *scroll = selected.saturating_sub(19);
                    }
                }
                return;
            }
            Overlay::CommitInput { input, .. } => {
                input.move_down();
                return;
            }
            Overlay::StashList { entries, selected } => {
                if *selected < entries.len().saturating_sub(1) {
                    *selected += 1;
                }
                return;
            }
            Overlay::BranchList { entries, selected, creating: None, .. } => {
                if *selected < entries.len().saturating_sub(1) {
                    *selected += 1;
                }
                return;
            }
            Overlay::BranchList { .. } => return,
            Overlay::Confirm { .. } => return,
            Overlay::None => {}
        }

        if self.active_panel == Panel::FileList {
            if !self.file_entries.is_empty()
                && self.selected_index < self.file_entries.len() - 1
            {
                self.selected_index += 1;
            }
            return;
        }
        let Some(ds) = &mut self.diff_state else { return };
        match ds.view_mode {
            DiffViewMode::HunkNav => {
                if ds.hunks.is_empty() {
                    if ds.scroll < ds.max_scroll {
                        ds.scroll += 1;
                    }
                } else if ds.current_hunk < ds.hunks.len() - 1 {
                    ds.current_hunk += 1;
                    ds.scroll = ds.hunks[ds.current_hunk].display_start;
                }
            }
            DiffViewMode::LineNav => {
                if let Some(pos) = ds.hunk_changed_rows.iter().position(|&r| r == ds.cursor_line) {
                    if pos < ds.hunk_changed_rows.len() - 1 {
                        ds.cursor_line = ds.hunk_changed_rows[pos + 1];
                        if ds.cursor_line >= ds.scroll + 20 {
                            ds.scroll = ds.cursor_line.saturating_sub(10);
                        }
                    }
                }
            }
        }
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

    fn stage_selected_lines(&mut self) -> Result<()> {
        let (path, old, new, selected, total) = {
            let Some(ds) = &self.diff_state else {
                self.status_message = Some("No diff to stage from".into());
                return Ok(());
            };
            if ds.view_mode != DiffViewMode::LineNav {
                return Ok(());
            }
            let lines = if ds.selected_lines.is_empty() {
                let mut s = BTreeSet::new();
                s.insert(ds.cursor_line);
                s
            } else {
                ds.selected_lines.clone()
            };
            let count = lines.len();
            (
                ds.file_path.clone(),
                ds.old_content.clone(),
                ds.new_content.clone(),
                lines,
                count,
            )
        };

        let patched = git::apply_lines(&old, &new, &selected);
        self.repo.stage_content(&path, &patched)?;
        self.status_message = Some(format!("Staged {total} line(s) of {path}"));
        self.refresh()?;
        self.load_selected_diff()?;
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        self.file_entries = self.repo.get_file_statuses()?;
        self.branch_name = self.repo.branch_name();
        self.ahead_behind = self.repo.ahead_behind();
        if self.selected_index >= self.file_entries.len() {
            self.selected_index = self.file_entries.len().saturating_sub(1);
        }
        Ok(())
    }

    pub fn filtered_entries(&self) -> Vec<(usize, &FileEntry)> {
        self.file_entries.iter().enumerate().filter(|(_, e)| {
            match &self.file_filter {
                None => true,
                Some(f) if f.is_empty() => true,
                Some(f) => {
                    let lower = e.path.to_lowercase();
                    f.to_lowercase().split_whitespace().all(|w| lower.contains(w))
                }
            }
        }).collect()
    }

    fn get_yank_text(&self) -> Option<String> {
        // In git log overlay, yank the selected commit hash
        if let Overlay::GitLog { entries, selected, .. } = &self.overlay {
            return entries.get(*selected).map(|e| e.hash.clone());
        }
        // In diff line mode, yank selected lines
        if let Some(ds) = &self.diff_state {
            if ds.view_mode == DiffViewMode::LineNav && !ds.selected_lines.is_empty() {
                let lines: Vec<String> = ds.selected_lines.iter()
                    .filter_map(|&row| ds.right_lines.get(row).map(|l| l.content.clone()))
                    .collect();
                if !lines.is_empty() {
                    return Some(lines.join("\n"));
                }
            }
        }
        // In file list, yank the file path
        if let Some(entry) = self.file_entries.get(self.selected_index) {
            return Some(entry.path.clone());
        }
        None
    }

    fn load_selected_diff(&mut self) -> Result<()> {
        if let Some(entry) = self.file_entries.get(self.selected_index) {
            let path = entry.path.clone();
            match self.repo.get_diff_content(&path) {
                Ok((old, new)) => {
                    let (left_lines, right_lines, line_mapping, hunks) =
                        git::compute_diff(&old, &new);
                    let max_scroll = left_lines.len().max(right_lines.len());
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
                        view_mode: DiffViewMode::HunkNav,
                        cursor_line: 0,
                        selected_lines: BTreeSet::new(),
                        hunk_changed_rows: Vec::new(),
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
