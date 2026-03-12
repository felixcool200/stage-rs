use crate::git::{self, BlameLine, BranchEntry, DiffLine, FileEntry, FileStatus, GitRepo, Hunk, LogEntry, StashEntry};
use crate::syntax::Highlighter;
use color_eyre::{eyre::eyre, Result};
use std::collections::BTreeSet;
use std::time::Instant;

pub struct App {
    pub repo: GitRepo,
    pub file_entries: Vec<FileEntry>,
    pub selected_index: usize,
    /// Whether the "Repository" header is selected (above all files)
    pub header_selected: bool,
    pub active_panel: Panel,
    pub diff_state: Option<DiffState>,
    pub branch_name: String,
    pub ahead_behind: (usize, usize),
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub last_refresh: Instant,
    pub overlay: Overlay,
    /// Saved commit message from undo, pre-filled on next commit
    pub saved_commit_msg: Option<String>,
    /// File list filter (None = not filtering)
    pub file_filter: Option<String>,
    /// Blame annotations for the current file (None = blame not loaded)
    pub blame_data: Option<Vec<BlameLine>>,
    pub show_blame: bool,
    /// Pending request to open $EDITOR
    pub pending_editor: Option<EditorRequest>,
    /// Which-key popup entries (None = popup closed)
    pub which_key: Option<Vec<WhichKeyEntry>>,
    /// Conflict resolver state
    pub conflict_state: Option<ConflictState>,
    /// Syntax highlighter
    pub highlighter: Highlighter,
}

pub struct ConflictState {
    pub file_path: String,
    pub sections: Vec<ConflictSection>,
    pub current_section: usize,
    /// Lines before first conflict
    pub prefix: Vec<String>,
    /// Branch name from <<<<<<< marker (e.g. "HEAD" or "master")
    pub left_name: String,
    /// Branch name from >>>>>>> marker (e.g. "feature-branch")
    pub right_name: String,
}

#[derive(Clone)]
pub struct ConflictSection {
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
    pub resolution: ConflictResolution,
    /// Lines between this conflict and the next
    pub suffix: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    Unresolved,
    Ours,
    Theirs,
    Both,
}

pub struct EditorRequest {
    pub file_path: String,
    pub line_number: usize,
}

pub struct DiffState {
    pub file_path: String,
    pub left_lines: Vec<DiffLine>,
    pub right_lines: Vec<DiffLine>,
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
    /// Remembered line selection from the last ExitLineMode (hunk_index, selection, cursor)
    pub saved_line_selection: Option<(usize, BTreeSet<usize>, usize)>,
    /// Last known viewport height (updated during render)
    pub viewport_height: usize,
    /// Whether this diff is for a staged file (HEAD vs index)
    pub is_staged: bool,
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
    CommitDetail {
        hash: String,
        message: String,
        diff_lines: Vec<String>,
        scroll: usize,
    },
    Rebase {
        entries: Vec<RebaseEntry>,
        selected: usize,
        base_hash: String,
    },
    DirtyCheckout {
        branch: String,
        has_conflicts: bool,
    },
}

#[derive(Debug, Clone)]
pub struct RebaseEntry {
    pub hash: String,
    pub message: String,
    pub action: RebaseAction,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RebaseAction {
    Pick,
    Squash,
    Drop,
}

impl RebaseAction {
    pub fn cycle(self) -> Self {
        match self {
            RebaseAction::Pick => RebaseAction::Squash,
            RebaseAction::Squash => RebaseAction::Drop,
            RebaseAction::Drop => RebaseAction::Pick,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RebaseAction::Pick => "pick",
            RebaseAction::Squash => "squash",
            RebaseAction::Drop => "drop",
        }
    }
}

#[derive(Debug, Clone)]
pub enum PendingAction {
    CommitAmend,
    UndoLastCommit,
    DiscardChanges { path: String },
    StageEntireFile { path: String },
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

fn copy_to_clipboard(text: &str) -> Result<()> {
    use std::io::Write;
    // Try wl-copy (Wayland), then xclip, then xsel
    let candidates = [
        ("wl-copy", vec![]),
        ("xclip", vec!["-selection", "clipboard"]),
        ("xsel", vec!["--clipboard", "--input"]),
    ];
    for (cmd, args) in &candidates {
        if let Ok(mut child) = std::process::Command::new(cmd)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = child.wait();
            return Ok(());
        }
    }
    Err(eyre!("No clipboard tool found (install xclip, xsel, or wl-copy)"))
}

fn char_to_byte_idx(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

// ── Messages ─────────────────────────────────────────────────────────────────

// ── Which-key popup ──────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WhichKeyEntry {
    pub key: char,
    pub label: &'static str,
    pub message: Message,
}

#[derive(Debug, Clone)]
pub enum Message {
    MoveUp,
    MoveDown,
    PrevHunk,
    NextHunk,
    SwitchPanel,
    StageFile,
    StageHunk,
    UnstageFile,
    DiscardChanges,
    Refresh,
    AutoRefresh,
    EnterLineMode,
    ExitLineMode,
    ToggleBlame,
    EnterEditMode,
    // Merge conflict resolution
    ConflictPickOurs,
    ConflictPickTheirs,
    ConflictPickBoth,
    ConflictSave,
    ToggleLine,
    StageLines,
    SelectAllLines,
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
    // Remote
    GitFetch,
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
    ViewCommitDetail,
    // Interactive rebase
    StartRebase,
    RebaseCycleAction,
    RebaseMoveUp,
    RebaseMoveDown,
    RebaseExecute,
    RebaseContinue,
    RebaseAbort,
    // Which-key
    OpenWhichKey,
    // Conflict resolver
    CloseConflict,
    // Overlay actions (handled by overlay key routing, not keymap)
    CloseOverlay,
    ConfirmCommit,
    ConfirmAction,
    DirtyCheckoutStash,
    DirtyCheckoutDiscard,
    Quit,
}

// ── App ──────────────────────────────────────────────────────────────────────

impl App {
    pub fn new(path: &str) -> Result<Self> {
        let repo = GitRepo::open(path)?;
        let branch_name = repo.branch_name();
        let ahead_behind = repo.ahead_behind();
        let file_entries = repo.get_file_statuses()?;

        let mut app = Self {
            repo,
            file_entries,
            selected_index: 0,
            header_selected: false,
            active_panel: Panel::FileList,
            diff_state: None,
            branch_name,
            ahead_behind,
            should_quit: false,
            status_message: None,
            last_refresh: Instant::now(),
            overlay: Overlay::None,
            saved_commit_msg: None,
            file_filter: None,
            blame_data: None,
            show_blame: false,
            pending_editor: None,
            which_key: None,
            conflict_state: None,
            highlighter: Highlighter::new(),
        };
        // Load diff for the initially selected file
        if !app.file_entries.is_empty() {
            let _ = app.load_selected_diff();
        }
        Ok(app)
    }

    /// Returns the currently selected file entry, or None if the header is selected.
    pub fn selected_file_entry(&self) -> Option<&FileEntry> {
        if self.header_selected {
            None
        } else {
            self.file_entries.get(self.selected_index)
        }
    }

    pub fn build_which_key_entries(&self) -> Vec<WhichKeyEntry> {
        use Message::*;
        let in_line_mode = self
            .diff_state
            .as_ref()
            .map(|ds| ds.view_mode == DiffViewMode::LineNav)
            .unwrap_or(false);

        let mut entries = Vec::new();

        let in_conflict = self.conflict_state.is_some();

        match (self.active_panel, in_line_mode) {
            (Panel::FileList, _) if self.header_selected => {
                // Repository-level commands
                entries.push(WhichKeyEntry { key: 'c', label: "commit", message: OpenCommit });
                entries.push(WhichKeyEntry { key: 'C', label: "amend", message: OpenCommitAmend });
                entries.push(WhichKeyEntry { key: 'z', label: "undo commit", message: UndoLastCommit });
                entries.push(WhichKeyEntry { key: 'l', label: "log", message: OpenGitLog });
                entries.push(WhichKeyEntry { key: 'f', label: "fetch", message: GitFetch });
                entries.push(WhichKeyEntry { key: 'b', label: "branches", message: OpenBranchList });
                entries.push(WhichKeyEntry { key: 'B', label: "blame", message: ToggleBlame });
                entries.push(WhichKeyEntry { key: 'S', label: "stash", message: StashSave });
                entries.push(WhichKeyEntry { key: 'w', label: "stash list", message: OpenStashList });
                entries.push(WhichKeyEntry { key: 'r', label: "refresh", message: Refresh });
                if self.repo.is_rebasing() {
                    entries.push(WhichKeyEntry { key: 'R', label: "rebase continue", message: RebaseContinue });
                    entries.push(WhichKeyEntry { key: 'A', label: "rebase abort", message: RebaseAbort });
                }
            }
            (Panel::FileList, _) => {
                // Per-file context-sensitive commands
                if let Some(entry) = self.selected_file_entry() {
                    match &entry.status {
                        FileStatus::Staged(_) => {
                            entries.push(WhichKeyEntry { key: 'u', label: "unstage", message: UnstageFile });
                        }
                        FileStatus::Conflict => {
                            // Conflict files auto-open resolver; no direct actions here
                        }
                        _ => {
                            // Modified, untracked, etc.
                            entries.push(WhichKeyEntry { key: 's', label: "stage", message: StageFile });
                            entries.push(WhichKeyEntry { key: 'd', label: "discard", message: DiscardChanges });
                        }
                    }
                    entries.push(WhichKeyEntry { key: 'y', label: "yank name", message: YankToClipboard });
                }
            }
            (Panel::DiffView, _) if in_conflict => {
                entries.push(WhichKeyEntry { key: 'o', label: "pick ours", message: ConflictPickOurs });
                entries.push(WhichKeyEntry { key: 't', label: "pick theirs", message: ConflictPickTheirs });
                entries.push(WhichKeyEntry { key: 'b', label: "pick both", message: ConflictPickBoth });
                entries.push(WhichKeyEntry { key: 's', label: "save & stage", message: ConflictSave });
                entries.push(WhichKeyEntry { key: 'r', label: "refresh", message: Refresh });
                if self.repo.is_rebasing() {
                    entries.push(WhichKeyEntry { key: 'R', label: "rebase continue", message: RebaseContinue });
                    entries.push(WhichKeyEntry { key: 'A', label: "rebase abort", message: RebaseAbort });
                }
            }
            (Panel::DiffView, false) => {
                entries.push(WhichKeyEntry { key: 's', label: "stage hunk", message: StageHunk });
                entries.push(WhichKeyEntry { key: 'i', label: "edit", message: EnterEditMode });
                entries.push(WhichKeyEntry { key: 'B', label: "blame", message: ToggleBlame });
                entries.push(WhichKeyEntry { key: 'y', label: "yank hunk", message: YankToClipboard });
                entries.push(WhichKeyEntry { key: 'r', label: "refresh", message: Refresh });
            }
            (Panel::DiffView, true) => {
                let is_staged = self.diff_state.as_ref().map(|ds| ds.is_staged).unwrap_or(false);
                entries.push(WhichKeyEntry { key: 'a', label: "toggle all", message: SelectAllLines });
                entries.push(WhichKeyEntry {
                    key: 's',
                    label: if is_staged { "unstage selected" } else { "stage selected" },
                    message: StageLines,
                });
                entries.push(WhichKeyEntry { key: 'i', label: "edit", message: EnterEditMode });
                entries.push(WhichKeyEntry { key: 'y', label: "yank lines", message: YankToClipboard });
                entries.push(WhichKeyEntry { key: 'r', label: "refresh", message: Refresh });
            }
        }
        entries
    }

    pub fn update(&mut self, msg: Message) -> Result<()> {
        match msg {
            Message::OpenWhichKey => {
                self.which_key = Some(self.build_which_key_entries());
            }
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
            Message::MoveUp => self.handle_move_up()?,
            Message::MoveDown => self.handle_move_down()?,
            Message::PrevHunk => {
                if let Some(ds) = &mut self.diff_state {
                    if !ds.hunks.is_empty() && ds.current_hunk > 0 {
                        ds.current_hunk -= 1;
                        let start = ds.hunks[ds.current_hunk].display_start;
                        let offset = ds.viewport_height / 3;
                        ds.scroll = start.saturating_sub(offset);
                    }
                }
            }
            Message::NextHunk => {
                if let Some(ds) = &mut self.diff_state {
                    if !ds.hunks.is_empty() && ds.current_hunk < ds.hunks.len() - 1 {
                        ds.current_hunk += 1;
                        let start = ds.hunks[ds.current_hunk].display_start;
                        let offset = ds.viewport_height / 3;
                        ds.scroll = start.saturating_sub(offset);
                    }
                }
            }
            Message::SwitchPanel => {
                if self.active_panel == Panel::DiffView {
                    if let Some(ds) = &mut self.diff_state {
                        ds.view_mode = DiffViewMode::HunkNav;
                        ds.selected_lines.clear();
                    }
                }
                // Don't switch to diff view when header is selected (no diff loaded)
                if self.header_selected && self.active_panel == Panel::FileList {
                    return Ok(());
                }
                self.active_panel = match self.active_panel {
                    Panel::FileList => Panel::DiffView,
                    Panel::DiffView => Panel::FileList,
                };
            }
            Message::StageFile => {
                if let Some(entry) = self.selected_file_entry() {
                    if entry.status == FileStatus::Conflict {
                        self.status_message = Some("Cannot stage conflict file — resolve conflicts first".into());
                        return Ok(());
                    }
                    let path = entry.path.clone();
                    // In diff view with hunks available, confirm before staging entire file
                    let in_diff_partial = self.active_panel == Panel::DiffView
                        && self.diff_state.as_ref().map_or(false, |ds| !ds.hunks.is_empty());
                    if in_diff_partial {
                        self.overlay = Overlay::Confirm {
                            message: format!("Stage entire file '{path}'? (Use 's' to stage current hunk)"),
                            action: PendingAction::StageEntireFile { path },
                        };
                    } else {
                        self.repo.stage_file(&path)?;
                        self.status_message = Some(format!("Staged: {path}"));
                        self.refresh()?;
                        self.load_selected_diff()?;
                    }
                }
            }
            Message::StageHunk => {
                self.stage_current_hunk()?;
            }
            Message::UnstageFile => {
                if let Some(entry) = self.selected_file_entry() {
                    let path = entry.path.clone();
                    self.repo.unstage_file(&path)?;
                    self.status_message = Some(format!("Unstaged: {path}"));
                    self.refresh()?;
                    self.load_selected_diff()?;
                }
            }
            Message::DiscardChanges => {
                if let Some(entry) = self.selected_file_entry() {
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
                    // Collect all changed rows across all hunks
                    let mut all_changed = Vec::new();
                    for hunk in &ds.hunks {
                        all_changed.extend(git::changed_rows_in_hunk(hunk, &ds.left_lines));
                    }
                    if all_changed.is_empty() {
                        return Ok(());
                    }

                    // Restore saved selection, or start at current hunk
                    if let Some((_saved_hunk, saved_sel, saved_cursor)) = ds.saved_line_selection.take() {
                        ds.selected_lines = saved_sel;
                        ds.cursor_line = saved_cursor;
                    } else {
                        ds.selected_lines.clear();
                        let hunk = &ds.hunks[ds.current_hunk];
                        let hunk_rows = git::changed_rows_in_hunk(hunk, &ds.left_lines);
                        ds.cursor_line = hunk_rows.first().copied().unwrap_or(all_changed[0]);
                    }
                    ds.hunk_changed_rows = all_changed;
                    ds.view_mode = DiffViewMode::LineNav;
                    self.status_message = None;
                    // Only adjust scroll if cursor is off-screen
                    Self::keep_cursor_visible(ds);
                }
            }
            Message::EnterEditMode => {
                if let Some(ds) = &self.diff_state {
                    let path = ds.file_path.clone();

                    // Map current diff scroll position to a file line number.
                    // Count non-spacer lines in right_lines up to the scroll position.
                    let scroll = ds.scroll;
                    let mut file_line: usize = 0;
                    for (i, dl) in ds.right_lines.iter().enumerate() {
                        if i >= scroll {
                            break;
                        }
                        if dl.kind != git::DiffLineKind::Spacer {
                            file_line += 1;
                        }
                    }
                    // Editor line numbers are 1-based
                    let line_number = file_line.max(1);

                    self.pending_editor = Some(EditorRequest {
                        file_path: path,
                        line_number,
                    });
                } else {
                    self.status_message = Some("Select a file first (Enter on file list)".into());
                }
            }
            Message::ConflictPickOurs => {
                if let Some(cs) = &mut self.conflict_state {
                    cs.sections[cs.current_section].resolution = ConflictResolution::Ours;
                    let name = &cs.left_name;
                    self.status_message = Some(format!("Picked: {name}"));
                }
            }
            Message::ConflictPickTheirs => {
                if let Some(cs) = &mut self.conflict_state {
                    cs.sections[cs.current_section].resolution = ConflictResolution::Theirs;
                    let name = &cs.right_name;
                    self.status_message = Some(format!("Picked: {name}"));
                }
            }
            Message::ConflictPickBoth => {
                if let Some(cs) = &mut self.conflict_state {
                    cs.sections[cs.current_section].resolution = ConflictResolution::Both;
                    self.status_message = Some("Picked: both".into());
                }
            }
            Message::ConflictSave => {
                if let Some(cs) = &self.conflict_state {
                    // Check all resolved
                    let unresolved = cs.sections.iter().filter(|s| s.resolution == ConflictResolution::Unresolved).count();
                    if unresolved > 0 {
                        self.status_message = Some(format!("{unresolved} conflict(s) still unresolved"));
                    } else {
                        let mut output = cs.prefix.clone();
                        for section in &cs.sections {
                            match section.resolution {
                                ConflictResolution::Ours => output.extend(section.ours.clone()),
                                ConflictResolution::Theirs => output.extend(section.theirs.clone()),
                                ConflictResolution::Both => {
                                    output.extend(section.ours.clone());
                                    output.extend(section.theirs.clone());
                                }
                                ConflictResolution::Unresolved => {}
                            }
                            output.extend(section.suffix.clone());
                        }
                        let content = output.join("\n") + "\n";
                        let path = cs.file_path.clone();
                        let workdir = self.repo.workdir().to_path_buf();
                        let full_path = workdir.join(&path);
                        match std::fs::write(&full_path, &content) {
                            Ok(()) => {
                                // Stage the resolved file
                                let _ = self.repo.stage_file(&path);
                                self.conflict_state = None;
                                self.status_message = Some(format!("Resolved and staged: {path}"));
                                self.refresh()?;
                                self.load_selected_diff()?;
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Save failed: {e}"));
                            }
                        }
                    }
                }
            }
            Message::CloseConflict => {
                self.active_panel = Panel::FileList;
            }
            Message::ToggleBlame => {
                if self.show_blame {
                    self.show_blame = false;
                    self.blame_data = None;
                    self.status_message = Some("Blame off".into());
                } else if let Some(ds) = &self.diff_state {
                    let path = ds.file_path.clone();
                    match self.repo.get_blame(&path) {
                        Ok(data) => {
                            self.blame_data = Some(data);
                            self.show_blame = true;
                            self.status_message = Some("Blame on".into());
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Blame failed: {e}"));
                        }
                    }
                } else {
                    self.status_message = Some("No file selected for blame".into());
                }
            }
            Message::ExitLineMode => {
                if let Some(ds) = &mut self.diff_state {
                    // Save selection so re-entering restores it
                    ds.saved_line_selection = Some((
                        ds.current_hunk,
                        ds.selected_lines.clone(),
                        ds.cursor_line,
                    ));
                    ds.view_mode = DiffViewMode::HunkNav;
                    ds.selected_lines.clear();
                    let offset = ds.viewport_height / 3;
                    ds.scroll = ds.hunks.get(ds.current_hunk)
                        .map(|h| h.display_start.saturating_sub(offset))
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
            Message::YankToClipboard => {
                let text = self.get_yank_text();
                if let Some(text) = text {
                    match copy_to_clipboard(&text) {
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

            // ── Remote ────────────────────────────────────────────────────
            Message::GitFetch => {
                self.status_message = Some("Fetching...".into());
                match self.repo.fetch() {
                    Ok(msg) => {
                        self.status_message = Some(format!("Fetch: {msg}"));
                        self.refresh()?;
                    }
                    Err(e) => self.status_message = Some(format!("Fetch failed: {e}")),
                }
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
                            entry.name.split('/').skip(1).collect::<Vec<_>>().join("/")
                        } else {
                            entry.name.clone()
                        };
                        if !self.file_entries.is_empty() {
                            let has_conflicts = self.file_entries.iter().any(|e| e.status == FileStatus::Conflict);
                            self.overlay = Overlay::DirtyCheckout { branch: name, has_conflicts };
                        } else {
                            self.overlay = Overlay::None;
                            self.do_checkout(&name)?;
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
                                let msg = format!("{e}");
                                if msg.contains("Unmerged") || msg.contains("merge") {
                                    self.status_message = Some("Cannot undo commit during a merge — resolve or abort the merge first".into());
                                } else {
                                    self.status_message = Some(format!("Undo failed: {e}"));
                                }
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
                    PendingAction::StageEntireFile { path } => {
                        self.repo.stage_file(&path)?;
                        self.status_message = Some(format!("Staged: {path}"));
                        self.refresh()?;
                        self.load_selected_diff()?;
                    }
                }
            }
            Message::DirtyCheckoutStash => {
                let branch = match &self.overlay {
                    Overlay::DirtyCheckout { branch, .. } => branch.clone(),
                    _ => return Ok(()),
                };
                self.overlay = Overlay::None;
                match self.repo.stash_save(None) {
                    Ok(()) => {
                        self.do_checkout(&branch)?;
                    }
                    Err(e) => {
                        self.status_message = Some(format!("Stash failed: {e}"));
                    }
                }
            }
            Message::DirtyCheckoutDiscard => {
                let branch = match &self.overlay {
                    Overlay::DirtyCheckout { branch, .. } => branch.clone(),
                    _ => return Ok(()),
                };
                self.overlay = Overlay::None;
                self.do_force_checkout(&branch)?;
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
            Message::StartRebase => {
                if let Overlay::GitLog { entries, selected, .. } = &self.overlay {
                    if *selected == 0 {
                        self.status_message = Some("Select a base commit (not the first one)".into());
                    } else {
                        // Commits from index 0 to selected-1 will be rebased onto selected
                        let base_hash = entries[*selected].hash.clone();
                        let rebase_entries: Vec<RebaseEntry> = entries[..*selected]
                            .iter()
                            .rev() // oldest first
                            .map(|e| RebaseEntry {
                                hash: e.hash.clone(),
                                message: e.message.clone(),
                                action: RebaseAction::Pick,
                            })
                            .collect();
                        self.overlay = Overlay::Rebase {
                            entries: rebase_entries,
                            selected: 0,
                            base_hash,
                        };
                    }
                }
            }
            Message::RebaseCycleAction => {
                if let Overlay::Rebase { entries, selected, .. } = &mut self.overlay {
                    entries[*selected].action = entries[*selected].action.cycle();
                }
            }
            Message::RebaseMoveUp => {
                if let Overlay::Rebase { entries, selected, .. } = &mut self.overlay {
                    if *selected > 0 {
                        entries.swap(*selected, *selected - 1);
                        *selected -= 1;
                    }
                }
            }
            Message::RebaseMoveDown => {
                if let Overlay::Rebase { entries, selected, .. } = &mut self.overlay {
                    if *selected < entries.len().saturating_sub(1) {
                        entries.swap(*selected, *selected + 1);
                        *selected += 1;
                    }
                }
            }
            Message::RebaseExecute => {
                if let Overlay::Rebase { entries, base_hash, .. } = &self.overlay {
                    let workdir = self.repo.workdir().to_path_buf();
                    // Build the rebase todo
                    let todo: String = entries
                        .iter()
                        .map(|e| format!("{} {} {}", e.action.label(), e.hash, e.message))
                        .collect::<Vec<_>>()
                        .join("\n");

                    // Write to a temp script that will be used as GIT_SEQUENCE_EDITOR
                    let todo_path = workdir.join(".git/stage-rebase-todo");
                    let _ = std::fs::write(&todo_path, &todo);

                    let script = format!(
                        "#!/bin/sh\ncp {} \"$1\"",
                        todo_path.display()
                    );
                    let script_path = workdir.join(".git/stage-rebase-editor.sh");
                    let _ = std::fs::write(&script_path, &script);
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let _ = std::fs::set_permissions(
                            &script_path,
                            std::fs::Permissions::from_mode(0o755),
                        );
                    }

                    let base = base_hash.clone();
                    self.overlay = Overlay::None;

                    let output = std::process::Command::new("git")
                        .args(["rebase", "-i", &base])
                        .env("GIT_SEQUENCE_EDITOR", script_path.to_str().unwrap_or(""))
                        .current_dir(&workdir)
                        .output();

                    // Cleanup
                    let _ = std::fs::remove_file(&todo_path);
                    let _ = std::fs::remove_file(&script_path);

                    match output {
                        Ok(o) if o.status.success() => {
                            self.status_message = Some("Rebase completed".into());
                            self.refresh()?;
                        }
                        Ok(o) => {
                            let err = String::from_utf8_lossy(&o.stderr).trim().to_string();
                            if self.repo.is_rebasing() {
                                self.status_message = Some("Rebase paused — resolve conflicts, then Space → R:continue / A:abort".into());
                                self.refresh()?;
                            } else {
                                self.status_message = Some(format!("Rebase failed: {err}"));
                            }
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Rebase error: {e}"));
                        }
                    }
                }
            }
            Message::RebaseContinue => {
                if self.repo.is_rebasing() {
                    match self.repo.rebase_continue() {
                        Ok(msg) => {
                            if self.repo.is_rebasing() {
                                self.status_message = Some("Rebase paused — resolve next conflict, then continue".into());
                            } else {
                                self.status_message = Some(msg);
                            }
                            self.refresh()?;
                            if self.diff_state.is_some() {
                                self.load_selected_diff()?;
                            }
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Rebase continue failed: {e}"));
                        }
                    }
                } else {
                    self.status_message = Some("No rebase in progress".into());
                }
            }
            Message::RebaseAbort => {
                if self.repo.is_rebasing() {
                    match self.repo.rebase_abort() {
                        Ok(msg) => {
                            self.status_message = Some(msg);
                            self.refresh()?;
                            if self.diff_state.is_some() {
                                self.load_selected_diff()?;
                            }
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Rebase abort failed: {e}"));
                        }
                    }
                } else {
                    self.status_message = Some("No rebase in progress".into());
                }
            }
            Message::ViewCommitDetail => {
                if let Overlay::GitLog { entries, selected, .. } = &self.overlay {
                    if let Some(entry) = entries.get(*selected) {
                        let hash = entry.hash.clone();
                        let message = entry.message.clone();
                        match self.repo.get_commit_diff(&hash) {
                            Ok(diff_text) => {
                                let diff_lines: Vec<String> = diff_text.lines().map(String::from).collect();
                                self.overlay = Overlay::CommitDetail {
                                    hash,
                                    message,
                                    diff_lines,
                                    scroll: 0,
                                };
                            }
                            Err(e) => {
                                self.status_message = Some(format!("Diff failed: {e}"));
                            }
                        }
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

    fn handle_move_up(&mut self) -> Result<()> {
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
                return Ok(());
            }
            Overlay::CommitInput { input, .. } => {
                input.move_up();
                return Ok(());
            }
            Overlay::StashList { selected, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                }
                return Ok(());
            }
            Overlay::BranchList { selected, creating: None, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                }
                return Ok(());
            }
            Overlay::BranchList { .. } => return Ok(()),
            Overlay::CommitDetail { scroll, .. } => {
                *scroll = scroll.saturating_sub(1);
                return Ok(());
            }
            Overlay::Rebase { selected, .. } => {
                if *selected > 0 {
                    *selected -= 1;
                }
                return Ok(());
            }
            Overlay::Confirm { .. } | Overlay::DirtyCheckout { .. } => return Ok(()),
            Overlay::None => {}
        }

        if self.active_panel == Panel::FileList {
            if self.header_selected {
                // Already at top, do nothing
            } else if self.selected_index > 0 {
                self.selected_index -= 1;
                self.load_selected_diff()?;
            } else {
                // At index 0, move to header
                self.header_selected = true;
                self.diff_state = None;
                self.conflict_state = None;
            }
            return Ok(());
        }
        if let Some(cs) = &mut self.conflict_state {
            if cs.current_section > 0 {
                cs.current_section -= 1;
            }
            return Ok(());
        }
        let Some(ds) = &mut self.diff_state else { return Ok(()) };
        match ds.view_mode {
            DiffViewMode::HunkNav => {
                ds.scroll = ds.scroll.saturating_sub(1);
                Self::update_current_hunk_from_scroll(ds);
            }
            DiffViewMode::LineNav => {
                if let Some(pos) = ds.hunk_changed_rows.iter().position(|&r| r == ds.cursor_line) {
                    if pos > 0 {
                        ds.cursor_line = ds.hunk_changed_rows[pos - 1];
                        Self::keep_cursor_visible(ds);
                        Self::update_current_hunk_from_cursor(ds);
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_move_down(&mut self) -> Result<()> {
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
                return Ok(());
            }
            Overlay::CommitInput { input, .. } => {
                input.move_down();
                return Ok(());
            }
            Overlay::StashList { entries, selected } => {
                if *selected < entries.len().saturating_sub(1) {
                    *selected += 1;
                }
                return Ok(());
            }
            Overlay::BranchList { entries, selected, creating: None, .. } => {
                if *selected < entries.len().saturating_sub(1) {
                    *selected += 1;
                }
                return Ok(());
            }
            Overlay::BranchList { .. } => return Ok(()),
            Overlay::CommitDetail { diff_lines, scroll, .. } => {
                if *scroll < diff_lines.len().saturating_sub(1) {
                    *scroll += 1;
                }
                return Ok(());
            }
            Overlay::Rebase { entries, selected, .. } => {
                if *selected < entries.len().saturating_sub(1) {
                    *selected += 1;
                }
                return Ok(());
            }
            Overlay::Confirm { .. } | Overlay::DirtyCheckout { .. } => return Ok(()),
            Overlay::None => {}
        }

        if self.active_panel == Panel::FileList {
            if self.header_selected {
                if !self.file_entries.is_empty() {
                    self.header_selected = false;
                    self.selected_index = 0;
                    self.load_selected_diff()?;
                }
            } else if !self.file_entries.is_empty()
                && self.selected_index < self.file_entries.len() - 1
            {
                self.selected_index += 1;
                self.load_selected_diff()?;
            }
            return Ok(());
        }
        if let Some(cs) = &mut self.conflict_state {
            if cs.current_section < cs.sections.len().saturating_sub(1) {
                cs.current_section += 1;
            }
            return Ok(());
        }
        let Some(ds) = &mut self.diff_state else { return Ok(()) };
        match ds.view_mode {
            DiffViewMode::HunkNav => {
                if ds.scroll < ds.max_scroll {
                    ds.scroll += 1;
                }
                Self::update_current_hunk_from_scroll(ds);
            }
            DiffViewMode::LineNav => {
                if let Some(pos) = ds.hunk_changed_rows.iter().position(|&r| r == ds.cursor_line) {
                    if pos < ds.hunk_changed_rows.len() - 1 {
                        ds.cursor_line = ds.hunk_changed_rows[pos + 1];
                        Self::keep_cursor_visible(ds);
                        Self::update_current_hunk_from_cursor(ds);
                    }
                }
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
        let (path, old, new, apply_rows, is_staged, display_count) = {
            let Some(ds) = &self.diff_state else {
                self.status_message = Some("No diff to stage from".into());
                return Ok(());
            };
            if ds.view_mode != DiffViewMode::LineNav {
                return Ok(());
            }
            if ds.selected_lines.is_empty() {
                self.status_message = Some("No lines selected — use Enter/→ to select lines first".into());
                return Ok(());
            }
            let user_selected = ds.selected_lines.clone();
            let count = user_selected.len();

            // For staged files, the user selects lines to *unstage*.
            // We invert: apply all changed rows except the selected ones.
            let apply_rows = if ds.is_staged {
                ds.hunk_changed_rows
                    .iter()
                    .copied()
                    .filter(|r| !user_selected.contains(r))
                    .collect::<BTreeSet<usize>>()
            } else {
                user_selected
            };

            (
                ds.file_path.clone(),
                ds.old_content.clone(),
                ds.new_content.clone(),
                apply_rows,
                ds.is_staged,
                count,
            )
        };

        let patched = git::apply_lines(&old, &new, &apply_rows);
        self.repo.stage_content(&path, &patched)?;
        let verb = if is_staged { "Unstaged" } else { "Staged" };
        self.status_message = Some(format!("{verb} {display_count} line(s) of {path}"));
        // Reset to hunk nav before reloading so stale line indices aren't preserved
        if let Some(ds) = &mut self.diff_state {
            ds.view_mode = DiffViewMode::HunkNav;
            ds.selected_lines.clear();
            ds.hunk_changed_rows.clear();
            ds.saved_line_selection = None;
        }
        self.refresh()?;
        self.load_selected_diff()?;
        Ok(())
    }

    fn refresh(&mut self) -> Result<()> {
        self.file_entries = self.repo.get_file_statuses()?;
        self.branch_name = self.repo.branch_name();
        self.ahead_behind = self.repo.ahead_behind();
        if !self.header_selected && self.selected_index >= self.file_entries.len() {
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
        if let Some(ds) = &self.diff_state {
            // In line mode, yank selected lines (or current line if none selected)
            if ds.view_mode == DiffViewMode::LineNav {
                let rows: Vec<usize> = if ds.selected_lines.is_empty() {
                    vec![ds.cursor_line]
                } else {
                    ds.selected_lines.iter().copied().collect()
                };
                let lines: Vec<String> = rows.iter()
                    .filter_map(|&row| ds.right_lines.get(row).map(|l| l.content.clone()))
                    .collect();
                if !lines.is_empty() {
                    return Some(lines.join("\n"));
                }
            }
            // In hunk mode, yank all changed lines in the current hunk
            if ds.view_mode == DiffViewMode::HunkNav {
                if let Some(hunk) = ds.hunks.get(ds.current_hunk) {
                    let lines: Vec<String> = ds.right_lines[hunk.display_start..hunk.display_end]
                        .iter()
                        .filter(|l| l.kind == git::DiffLineKind::Added || l.kind == git::DiffLineKind::Equal)
                        .map(|l| l.content.clone())
                        .collect();
                    if !lines.is_empty() {
                        return Some(lines.join("\n"));
                    }
                }
            }
        }
        // In file list, yank the file path
        if let Some(entry) = self.selected_file_entry() {
            return Some(entry.path.clone());
        }
        None
    }


    /// Scroll just enough to keep cursor_line visible, placing it 1/3 from top/bottom edge.
    fn keep_cursor_visible(ds: &mut DiffState) {
        let margin = ds.viewport_height / 3;
        if ds.cursor_line < ds.scroll + margin {
            ds.scroll = ds.cursor_line.saturating_sub(margin);
        } else if ds.cursor_line >= ds.scroll + ds.viewport_height.saturating_sub(margin) {
            ds.scroll = ds.cursor_line.saturating_sub(ds.viewport_height.saturating_sub(margin).saturating_sub(1));
        }
    }

    /// Update current_hunk to match whichever hunk the cursor is inside.
    fn update_current_hunk_from_cursor(ds: &mut DiffState) {
        for (i, hunk) in ds.hunks.iter().enumerate() {
            if ds.cursor_line >= hunk.display_start && ds.cursor_line < hunk.display_end {
                ds.current_hunk = i;
                return;
            }
        }
    }

    /// Update current_hunk to the hunk nearest the top visible area during scroll.
    fn update_current_hunk_from_scroll(ds: &mut DiffState) {
        // Use 1/3 offset only when there's room to scroll up; otherwise use the
        // actual top of the viewport so early hunks in short files aren't skipped.
        let offset = ds.viewport_height / 3;
        let focus_line = if ds.scroll >= offset {
            ds.scroll + offset
        } else {
            ds.scroll
        };
        // Find the hunk that contains focus_line, or the nearest one after it
        for (i, hunk) in ds.hunks.iter().enumerate() {
            if focus_line < hunk.display_end {
                ds.current_hunk = i;
                return;
            }
        }
        // Past all hunks — select the last one
        if !ds.hunks.is_empty() {
            ds.current_hunk = ds.hunks.len() - 1;
        }
    }

    fn do_checkout(&mut self, name: &str) -> Result<()> {
        match self.repo.checkout_branch(name) {
            Ok(()) => {
                self.status_message = Some(format!("Switched to {name}"));
                self.reset_after_checkout()?;
            }
            Err(e) => {
                self.status_message = Some(format!("Checkout failed: {e}"));
            }
        }
        Ok(())
    }

    fn do_force_checkout(&mut self, name: &str) -> Result<()> {
        match self.repo.force_checkout_branch(name) {
            Ok(()) => {
                self.status_message = Some(format!("Switched to {name} (changes discarded)"));
                self.reset_after_checkout()?;
            }
            Err(e) => {
                self.status_message = Some(format!("Checkout failed: {e}"));
            }
        }
        Ok(())
    }

    fn reset_after_checkout(&mut self) -> Result<()> {
        self.diff_state = None;
        self.conflict_state = None;
        self.blame_data = None;
        self.selected_index = 0;
        self.header_selected = false;
        self.active_panel = Panel::FileList;
        self.refresh()?;
        Ok(())
    }

    fn load_selected_diff(&mut self) -> Result<()> {
        if let Some(entry) = self.selected_file_entry() {
            let path = entry.path.clone();
            let status = entry.status.clone();

            // Auto-open conflict resolver for conflict files
            if status == FileStatus::Conflict {
                self.diff_state = None;
                let workdir = self.repo.workdir().to_path_buf();
                let full_path = workdir.join(&path);
                if let Ok(content) = std::fs::read_to_string(&full_path) {
                    if let Some(parsed) = parse_conflicts(&content) {
                        self.conflict_state = Some(ConflictState {
                            file_path: path,
                            sections: parsed.sections,
                            current_section: 0,
                            prefix: parsed.prefix,
                            left_name: parsed.left_name.clone(),
                            right_name: parsed.right_name.clone(),
                        });
                        self.status_message = Some(
                            "Conflict: Space=actions  ↑/↓=navigate  ←/Esc=back".into(),
                        );
                        return Ok(());
                    }
                }
                return Ok(());
            }

            // Close conflict resolver when navigating away from a conflict file
            if self.conflict_state.is_some() {
                self.conflict_state = None;
            }

            // Refresh blame data when file changes
            let file_changed = self.diff_state.as_ref().map(|ds| ds.file_path != path).unwrap_or(true);
            if file_changed && self.show_blame {
                self.blame_data = self.repo.get_blame(&path).ok();
            }

            let staged = matches!(status, FileStatus::Staged(_));
            match self.repo.get_diff_content(&path, staged) {
                Ok((old, new)) => {
                    let (left_lines, right_lines, hunks) =
                        git::compute_diff(&old, &new);
                    let max_scroll = left_lines.len().max(right_lines.len());
                    // Preserve state when reloading the same file
                    let prev = self.diff_state.as_ref().filter(|ds| ds.file_path == path);
                    let prev_hunk = prev
                        .map(|ds| ds.current_hunk.min(hunks.len().saturating_sub(1)))
                        .unwrap_or(0);
                    let prev_view_mode = prev.map(|ds| ds.view_mode).unwrap_or(DiffViewMode::HunkNav);
                    let prev_cursor = prev.map(|ds| ds.cursor_line).unwrap_or(0);
                    let prev_selected = prev.map(|ds| ds.selected_lines.clone()).unwrap_or_default();
                    let prev_hunk_rows = prev.map(|ds| ds.hunk_changed_rows.clone()).unwrap_or_default();
                    let prev_viewport = prev.map(|ds| ds.viewport_height).unwrap_or(24);
                    let offset = prev_viewport / 3;
                    let scroll = hunks
                        .get(prev_hunk)
                        .map(|h| h.display_start.saturating_sub(offset))
                        .unwrap_or(0);
                    let prev_saved = prev.and_then(|ds| ds.saved_line_selection.clone());
                    self.diff_state = Some(DiffState {
                        file_path: path,
                        left_lines,
                        right_lines,
                        hunks,
                        current_hunk: prev_hunk,
                        scroll,
                        max_scroll,
                        old_content: old,
                        new_content: new,
                        view_mode: prev_view_mode,
                        cursor_line: prev_cursor,
                        selected_lines: prev_selected,
                        hunk_changed_rows: prev_hunk_rows,
                        saved_line_selection: prev_saved,
                        viewport_height: prev_viewport,
                        is_staged: staged,
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

struct ParsedConflicts {
    prefix: Vec<String>,
    sections: Vec<ConflictSection>,
    left_name: String,
    right_name: String,
}

fn parse_conflicts(content: &str) -> Option<ParsedConflicts> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();
    let mut prefix = Vec::new();
    let mut i = 0;
    let mut in_prefix = true;
    let mut left_name = String::new();
    let mut right_name = String::new();

    while i < lines.len() {
        if lines[i].starts_with("<<<<<<<") {
            in_prefix = false;
            // Extract left branch name from "<<<<<<< NAME"
            if left_name.is_empty() {
                left_name = lines[i].trim_start_matches('<').trim().to_string();
            }
            let mut ours = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("=======") {
                ours.push(lines[i].to_string());
                i += 1;
            }
            i += 1; // skip =======
            let mut theirs = Vec::new();
            while i < lines.len() && !lines[i].starts_with(">>>>>>>") {
                theirs.push(lines[i].to_string());
                i += 1;
            }
            // Extract right branch name from ">>>>>>> NAME"
            if i < lines.len() && right_name.is_empty() {
                right_name = lines[i].trim_start_matches('>').trim().to_string();
            }
            i += 1; // skip >>>>>>>
            // Collect suffix lines until next conflict or end
            let mut suffix = Vec::new();
            while i < lines.len() && !lines[i].starts_with("<<<<<<<") {
                suffix.push(lines[i].to_string());
                i += 1;
            }
            sections.push(ConflictSection {
                ours,
                theirs,
                resolution: ConflictResolution::Unresolved,
                suffix,
            });
        } else {
            if in_prefix {
                prefix.push(lines[i].to_string());
            }
            i += 1;
        }
    }

    if sections.is_empty() {
        None
    } else {
        if left_name.is_empty() {
            left_name = "left".to_string();
        }
        if right_name.is_empty() {
            right_name = "right".to_string();
        }
        Some(ParsedConflicts {
            prefix,
            sections,
            left_name,
            right_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TextInput ──

    #[test]
    fn test_text_input_new_empty() {
        let ti = TextInput::new("");
        assert_eq!(ti.lines, vec![""]);
        assert_eq!(ti.cursor_row, 0);
        assert_eq!(ti.cursor_col, 0);
    }

    #[test]
    fn test_text_input_new_single_line() {
        let ti = TextInput::new("hello");
        assert_eq!(ti.lines, vec!["hello"]);
        assert_eq!(ti.cursor_row, 0);
        assert_eq!(ti.cursor_col, 5);
    }

    #[test]
    fn test_text_input_new_multiline() {
        let ti = TextInput::new("first\nsecond\nthird");
        assert_eq!(ti.lines, vec!["first", "second", "third"]);
        assert_eq!(ti.cursor_row, 2);
        assert_eq!(ti.cursor_col, 5);
    }

    #[test]
    fn test_text_input_insert_char() {
        let mut ti = TextInput::new("");
        ti.insert_char('a');
        ti.insert_char('b');
        assert_eq!(ti.to_string(), "ab");
        assert_eq!(ti.cursor_col, 2);
    }

    #[test]
    fn test_text_input_insert_char_mid_line() {
        let mut ti = TextInput::new("ac");
        ti.cursor_col = 1; // between a and c
        ti.insert_char('b');
        assert_eq!(ti.to_string(), "abc");
        assert_eq!(ti.cursor_col, 2);
    }

    #[test]
    fn test_text_input_insert_newline() {
        let mut ti = TextInput::new("hello world");
        ti.cursor_col = 5;
        ti.insert_newline();
        assert_eq!(ti.lines, vec!["hello", " world"]);
        assert_eq!(ti.cursor_row, 1);
        assert_eq!(ti.cursor_col, 0);
    }

    #[test]
    fn test_text_input_backspace_mid_line() {
        let mut ti = TextInput::new("abc");
        ti.cursor_col = 2;
        ti.backspace();
        assert_eq!(ti.to_string(), "ac");
        assert_eq!(ti.cursor_col, 1);
    }

    #[test]
    fn test_text_input_backspace_start_of_line() {
        let mut ti = TextInput::new("first\nsecond");
        ti.cursor_row = 1;
        ti.cursor_col = 0;
        ti.backspace();
        assert_eq!(ti.lines, vec!["firstsecond"]);
        assert_eq!(ti.cursor_row, 0);
        assert_eq!(ti.cursor_col, 5);
    }

    #[test]
    fn test_text_input_backspace_at_start_does_nothing() {
        let mut ti = TextInput::new("hello");
        ti.cursor_col = 0;
        ti.backspace();
        assert_eq!(ti.to_string(), "hello");
    }

    #[test]
    fn test_text_input_move_left() {
        let mut ti = TextInput::new("abc");
        ti.cursor_col = 2;
        ti.move_left();
        assert_eq!(ti.cursor_col, 1);
        ti.move_left();
        assert_eq!(ti.cursor_col, 0);
        ti.move_left(); // at start, should not go negative
        assert_eq!(ti.cursor_col, 0);
    }

    #[test]
    fn test_text_input_move_right() {
        let mut ti = TextInput::new("abc");
        ti.cursor_col = 0;
        ti.move_right();
        assert_eq!(ti.cursor_col, 1);
        ti.move_right();
        ti.move_right();
        assert_eq!(ti.cursor_col, 3);
        ti.move_right(); // at end, should not go past
        assert_eq!(ti.cursor_col, 3);
    }

    #[test]
    fn test_text_input_move_up_down() {
        let mut ti = TextInput::new("short\nlong line here");
        ti.cursor_row = 1;
        ti.cursor_col = 14;
        ti.move_up();
        assert_eq!(ti.cursor_row, 0);
        assert_eq!(ti.cursor_col, 5); // clamped to shorter line
        ti.move_down();
        assert_eq!(ti.cursor_row, 1);
        assert_eq!(ti.cursor_col, 5); // stays clamped
    }

    #[test]
    fn test_text_input_move_up_at_top() {
        let mut ti = TextInput::new("only");
        ti.cursor_row = 0;
        ti.move_up();
        assert_eq!(ti.cursor_row, 0);
    }

    #[test]
    fn test_text_input_move_down_at_bottom() {
        let mut ti = TextInput::new("only");
        ti.move_down();
        assert_eq!(ti.cursor_row, 0);
    }

    #[test]
    fn test_text_input_move_home_end() {
        let mut ti = TextInput::new("hello");
        ti.cursor_col = 3;
        ti.move_home();
        assert_eq!(ti.cursor_col, 0);
        ti.move_end();
        assert_eq!(ti.cursor_col, 5);
    }

    #[test]
    fn test_text_input_is_empty() {
        assert!(TextInput::new("").is_empty());
        assert!(TextInput::new("   ").is_empty());
        assert!(TextInput::new("  \n  \n  ").is_empty());
        assert!(!TextInput::new("hello").is_empty());
        assert!(!TextInput::new("\nhello\n").is_empty());
    }

    #[test]
    fn test_text_input_to_string() {
        let ti = TextInput::new("line1\nline2");
        assert_eq!(ti.to_string(), "line1\nline2");
    }

    #[test]
    fn test_text_input_unicode() {
        let mut ti = TextInput::new("héllo");
        // new() uses .len() (byte length) for initial cursor_col
        assert_eq!(ti.cursor_col, 6);
        // insert_char works with char indexing
        ti.cursor_col = 1;
        ti.insert_char('x');
        assert_eq!(ti.to_string(), "hxéllo");
    }

    // ── char_to_byte_idx ──

    #[test]
    fn test_char_to_byte_idx_ascii() {
        assert_eq!(char_to_byte_idx("hello", 0), 0);
        assert_eq!(char_to_byte_idx("hello", 3), 3);
        assert_eq!(char_to_byte_idx("hello", 5), 5);
    }

    #[test]
    fn test_char_to_byte_idx_unicode() {
        let s = "héllo"; // é is 2 bytes in UTF-8
        assert_eq!(char_to_byte_idx(s, 0), 0);
        assert_eq!(char_to_byte_idx(s, 1), 1); // 'h'
        assert_eq!(char_to_byte_idx(s, 2), 3); // after 'é' (2 bytes)
    }

    #[test]
    fn test_char_to_byte_idx_past_end() {
        assert_eq!(char_to_byte_idx("hi", 10), 2); // returns s.len()
    }

    // ── RebaseAction ──

    #[test]
    fn test_rebase_action_cycle() {
        assert_eq!(RebaseAction::Pick.cycle(), RebaseAction::Squash);
        assert_eq!(RebaseAction::Squash.cycle(), RebaseAction::Drop);
        assert_eq!(RebaseAction::Drop.cycle(), RebaseAction::Pick);
    }

    #[test]
    fn test_rebase_action_label() {
        assert_eq!(RebaseAction::Pick.label(), "pick");
        assert_eq!(RebaseAction::Squash.label(), "squash");
        assert_eq!(RebaseAction::Drop.label(), "drop");
    }

    #[test]
    fn test_rebase_action_full_cycle() {
        let action = RebaseAction::Pick;
        let cycled = action.cycle().cycle().cycle();
        assert_eq!(cycled, RebaseAction::Pick);
    }

    // ── parse_conflicts ──

    #[test]
    fn test_parse_conflicts_no_conflicts() {
        assert!(parse_conflicts("normal file content\nno conflicts here\n").is_none());
    }

    #[test]
    fn test_parse_conflicts_single_conflict() {
        let content = "before\n<<<<<<< HEAD\nours line\n=======\ntheirs line\n>>>>>>> branch\nafter\n";
        let parsed = parse_conflicts(content).unwrap();
        assert_eq!(parsed.prefix, vec!["before"]);
        assert_eq!(parsed.sections.len(), 1);
        assert_eq!(parsed.sections[0].ours, vec!["ours line"]);
        assert_eq!(parsed.sections[0].theirs, vec!["theirs line"]);
        assert_eq!(parsed.sections[0].resolution, ConflictResolution::Unresolved);
        assert_eq!(parsed.sections[0].suffix, vec!["after"]);
        assert_eq!(parsed.left_name, "HEAD");
        assert_eq!(parsed.right_name, "branch");
    }

    #[test]
    fn test_parse_conflicts_multiple_conflicts() {
        let content = "\
prefix line
<<<<<<< HEAD
ours1
=======
theirs1
>>>>>>> feature-branch
middle
<<<<<<< HEAD
ours2
=======
theirs2
>>>>>>> feature-branch
end";
        let parsed = parse_conflicts(content).unwrap();
        assert_eq!(parsed.prefix, vec!["prefix line"]);
        assert_eq!(parsed.left_name, "HEAD");
        assert_eq!(parsed.right_name, "feature-branch");
        assert_eq!(parsed.sections.len(), 2);
        assert_eq!(parsed.sections[0].ours, vec!["ours1"]);
        assert_eq!(parsed.sections[0].theirs, vec!["theirs1"]);
        assert_eq!(parsed.sections[0].suffix, vec!["middle"]);
        assert_eq!(parsed.sections[1].ours, vec!["ours2"]);
        assert_eq!(parsed.sections[1].theirs, vec!["theirs2"]);
        assert_eq!(parsed.sections[1].suffix, vec!["end"]);
    }

    #[test]
    fn test_parse_conflicts_multi_line_sides() {
        let content = "<<<<<<< HEAD\nour line 1\nour line 2\n=======\ntheir line 1\ntheir line 2\ntheir line 3\n>>>>>>> branch\n";
        let parsed = parse_conflicts(content).unwrap();
        assert!(parsed.prefix.is_empty());
        assert_eq!(parsed.sections[0].ours, vec!["our line 1", "our line 2"]);
        assert_eq!(parsed.sections[0].theirs, vec!["their line 1", "their line 2", "their line 3"]);
    }

    #[test]
    fn test_parse_conflicts_empty_sides() {
        let content = "<<<<<<< HEAD\n=======\ntheirs\n>>>>>>> branch\n";
        let parsed = parse_conflicts(content).unwrap();
        assert!(parsed.sections[0].ours.is_empty());
        assert_eq!(parsed.sections[0].theirs, vec!["theirs"]);
    }

    // ── Overlay ──

    #[test]
    fn test_overlay_is_active() {
        assert!(!Overlay::None.is_active());
        assert!(Overlay::Confirm {
            message: "test".into(),
            action: PendingAction::UndoLastCommit,
        }
        .is_active());
    }
}
