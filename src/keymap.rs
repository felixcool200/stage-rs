use crate::app::Message;
use crate::theme::Theme;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Style;
use ratatui::text::{Line, Span};

// ── Data structures ──────────────────────────────────────────────────────────

/// Context passed to the keymap resolver so it knows which mode we're in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputContext {
    FileList,
    DiffHunkNav,
    DiffLineNav,
    ConflictNav,
    // Overlay contexts
    Confirm,
    CommitInput,
    GitLog,
    StashList,
    BranchList,
    CommitDetail,
    Rebase,
}

/// A key combination (code + modifiers).
pub struct KeyBind {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBind {
    pub const fn plain(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }
    pub const fn ctrl(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
        }
    }
    pub const fn shift(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::SHIFT,
        }
    }
    pub const fn ctrl_code(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: KeyModifiers::CONTROL,
        }
    }

    fn matches(&self, key: &KeyEvent) -> bool {
        key.code == self.code && key.modifiers == self.modifiers
    }
}

/// A single keybinding: one or more key alternatives → one message.
pub struct Binding {
    pub keys: &'static [KeyBind],
    pub label: &'static str,
    pub message: Message,
    pub show_in_hint: bool,
}

impl Binding {
    fn matches(&self, key: &KeyEvent) -> bool {
        self.keys.iter().any(|kb| kb.matches(key))
    }
}

// ── Key display ──────────────────────────────────────────────────────────────

fn display_key(kb: &KeyBind) -> String {
    let key_str = match kb.code {
        KeyCode::Down => "↓".to_string(),
        KeyCode::Up => "↑".to_string(),
        KeyCode::Left => "←".to_string(),
        KeyCode::Right => "→".to_string(),
        KeyCode::Enter => "Enter".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Tab => "Tab".to_string(),
        _ => format!("{:?}", kb.code),
    };

    if kb.modifiers.contains(KeyModifiers::CONTROL) {
        format!("Ctrl+{}", key_str.to_uppercase())
    } else if kb.modifiers.contains(KeyModifiers::SHIFT) {
        format!("Shift+{key_str}")
    } else {
        key_str
    }
}

fn display_keys(keys: &[KeyBind]) -> String {
    keys.iter().map(display_key).collect::<Vec<_>>().join("/")
}

/// Merge key display strings that share a modifier prefix (e.g. "Shift+↑", "Shift+↓" → "Shift+↑/↓").
fn merge_key_displays(key_strs: &[String]) -> String {
    if key_strs.len() <= 1 {
        return key_strs.first().cloned().unwrap_or_default();
    }
    for prefix in &["Shift+", "Ctrl+"] {
        if key_strs.iter().all(|k| k.starts_with(prefix)) {
            let suffixes: Vec<&str> = key_strs.iter().map(|k| &k[prefix.len()..]).collect();
            return format!("{}{}", prefix, suffixes.join("/"));
        }
    }
    key_strs.join("/")
}

// ── Binding tables ───────────────────────────────────────────────────────────

use KeyCode::*;
use Message::*;

static FILE_LIST: &[Binding] = &[
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "navigate",
        message: MoveUp,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "navigate",
        message: MoveDown,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Enter), KeyBind::plain(Right)],
        label: "diff",
        message: SwitchPanel,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char('/'))],
        label: "filter",
        message: StartFilter,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char('q'))],
        label: "quit",
        message: Quit,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Char(' '))],
        label: "commands",
        message: OpenWhichKey,
        show_in_hint: true,
    },
];

static DIFF_HUNK_NAV: &[Binding] = &[
    Binding {
        keys: &[KeyBind::shift(Down)],
        label: "scroll",
        message: MoveDown,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::shift(Up)],
        label: "scroll",
        message: MoveUp,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "hunks",
        message: PrevHunk,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "hunks",
        message: NextHunk,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::shift(Left)],
        label: "scroll",
        message: ScrollLeft,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::shift(Right)],
        label: "scroll",
        message: ScrollRight,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Left)],
        label: "files",
        message: SwitchPanel,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Enter), KeyBind::plain(Right)],
        label: "lines",
        message: EnterLineMode,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char('q'))],
        label: "quit",
        message: Quit,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Char(' '))],
        label: "commands",
        message: OpenWhichKey,
        show_in_hint: true,
    },
];

static DIFF_LINE_NAV: &[Binding] = &[
    Binding {
        keys: &[KeyBind::shift(Down)],
        label: "next hunk",
        message: NextHunk,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::shift(Up)],
        label: "prev hunk",
        message: PrevHunk,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::shift(Left)],
        label: "scroll ←",
        message: ScrollLeft,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::shift(Right)],
        label: "scroll →",
        message: ScrollRight,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "lines",
        message: MoveUp,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "lines",
        message: MoveDown,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Enter), KeyBind::plain(Right)],
        label: "toggle",
        message: ToggleLine,
        show_in_hint: true,
    },
    Binding {
        keys: &[
            KeyBind::plain(Esc),
            KeyBind::plain(Left),
            KeyBind::plain(Char('q')),
        ],
        label: "back",
        message: ExitLineMode,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char(' '))],
        label: "commands",
        message: OpenWhichKey,
        show_in_hint: true,
    },
];

static CONFLICT_NAV: &[Binding] = &[
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "navigate",
        message: MoveUp,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "navigate",
        message: MoveDown,
        show_in_hint: true,
    },
    Binding {
        keys: &[
            KeyBind::plain(Left),
            KeyBind::plain(Esc),
            KeyBind::plain(Char('q')),
        ],
        label: "back",
        message: CloseConflict,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char(' '))],
        label: "actions",
        message: OpenWhichKey,
        show_in_hint: true,
    },
];

static CONFIRM: &[Binding] = &[
    Binding {
        keys: &[KeyBind::plain(Char('y')), KeyBind::plain(Enter)],
        label: "yes",
        message: ConfirmAction,
        show_in_hint: true,
    },
    Binding {
        keys: &[
            KeyBind::plain(Char('n')),
            KeyBind::plain(Esc),
            KeyBind::plain(Char('q')),
        ],
        label: "no",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

static COMMIT_INPUT: &[Binding] = &[
    Binding {
        keys: &[KeyBind::ctrl('c')],
        label: "cancel",
        message: CloseOverlay,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::ctrl('s'), KeyBind::ctrl('d')],
        label: "commit",
        message: ConfirmCommit,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Esc)],
        label: "cancel",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

static GIT_LOG: &[Binding] = &[
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "navigate",
        message: MoveDown,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "navigate",
        message: MoveUp,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Char('y'))],
        label: "yank hash",
        message: YankToClipboard,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Enter)],
        label: "view",
        message: ViewCommitDetail,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char('r'))],
        label: "rebase",
        message: StartRebase,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Esc), KeyBind::plain(Char('q'))],
        label: "close",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

static STASH_LIST: &[Binding] = &[
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "navigate",
        message: MoveDown,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "navigate",
        message: MoveUp,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Char('p')), KeyBind::plain(Enter)],
        label: "pop",
        message: StashPop,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char('a'))],
        label: "apply",
        message: StashApply,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char('d'))],
        label: "drop",
        message: StashDrop,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Esc), KeyBind::plain(Char('q'))],
        label: "close",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

static BRANCH_LIST: &[Binding] = &[
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "navigate",
        message: MoveDown,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "navigate",
        message: MoveUp,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Enter)],
        label: "checkout",
        message: CheckoutBranch,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Char('n'))],
        label: "new",
        message: StartCreateBranch,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Esc), KeyBind::plain(Char('q'))],
        label: "close",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

static COMMIT_DETAIL: &[Binding] = &[
    Binding {
        keys: &[KeyBind::ctrl_code(Down)],
        label: "commit",
        message: NextCommitDetail,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::ctrl_code(Up)],
        label: "commit",
        message: PrevCommitDetail,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::shift(Down)],
        label: "scroll",
        message: MoveDown,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::shift(Up)],
        label: "scroll",
        message: MoveUp,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "hunk",
        message: NextHunkCommitDetail,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "hunk",
        message: PrevHunkCommitDetail,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Esc), KeyBind::plain(Char('q'))],
        label: "close",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

static REBASE: &[Binding] = &[
    Binding {
        keys: &[KeyBind::shift(Down)],
        label: "reorder",
        message: RebaseMoveDown,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::shift(Up)],
        label: "reorder",
        message: RebaseMoveUp,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Down)],
        label: "navigate",
        message: MoveDown,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Up)],
        label: "navigate",
        message: MoveUp,
        show_in_hint: false,
    },
    Binding {
        keys: &[KeyBind::plain(Char(' ')), KeyBind::plain(Char('c'))],
        label: "cycle",
        message: RebaseCycleAction,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Enter)],
        label: "execute",
        message: RebaseExecute,
        show_in_hint: true,
    },
    Binding {
        keys: &[KeyBind::plain(Esc), KeyBind::plain(Char('q'))],
        label: "cancel",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

// DirtyCheckout is handled separately due to conditional 's' key.
static DIRTY_CHECKOUT_BASE: &[Binding] = &[
    Binding {
        keys: &[KeyBind::plain(Char('d'))],
        label: "discard & switch",
        message: DirtyCheckoutDiscard,
        show_in_hint: true,
    },
    Binding {
        keys: &[
            KeyBind::plain(Esc),
            KeyBind::plain(Char('c')),
            KeyBind::plain(Char('q')),
        ],
        label: "cancel",
        message: CloseOverlay,
        show_in_hint: true,
    },
];

static DIRTY_CHECKOUT_STASH: Binding = Binding {
    keys: &[KeyBind::plain(Char('s'))],
    label: "stash & switch",
    message: DirtyCheckoutStash,
    show_in_hint: true,
};

// ── Resolve ──────────────────────────────────────────────────────────────────

fn bindings_for(ctx: InputContext) -> &'static [Binding] {
    match ctx {
        InputContext::FileList => FILE_LIST,
        InputContext::DiffHunkNav => DIFF_HUNK_NAV,
        InputContext::DiffLineNav => DIFF_LINE_NAV,
        InputContext::ConflictNav => CONFLICT_NAV,
        InputContext::Confirm => CONFIRM,
        InputContext::CommitInput => COMMIT_INPUT,
        InputContext::GitLog => GIT_LOG,
        InputContext::StashList => STASH_LIST,
        InputContext::BranchList => BRANCH_LIST,
        InputContext::CommitDetail => COMMIT_DETAIL,
        InputContext::Rebase => REBASE,
    }
}

fn is_overlay(ctx: InputContext) -> bool {
    matches!(
        ctx,
        InputContext::Confirm
            | InputContext::CommitInput
            | InputContext::GitLog
            | InputContext::StashList
            | InputContext::BranchList
            | InputContext::CommitDetail
            | InputContext::Rebase
    )
}

/// Resolve a key event to a message given the current context.
pub fn resolve(ctx: InputContext, key: KeyEvent) -> Option<Message> {
    // Global Ctrl+C → Quit (non-overlay contexts only)
    if !is_overlay(ctx) && key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c')
    {
        return Some(Message::Quit);
    }

    // Table lookup
    for binding in bindings_for(ctx) {
        if binding.matches(&key) {
            return Some(binding.message);
        }
    }

    None
}

/// Resolve keys for the DirtyCheckout overlay (conditional 's' key).
pub fn resolve_dirty_checkout(key: KeyEvent, has_conflicts: bool) -> Option<Message> {
    if !has_conflicts && DIRTY_CHECKOUT_STASH.matches(&key) {
        return Some(DIRTY_CHECKOUT_STASH.message);
    }
    for binding in DIRTY_CHECKOUT_BASE {
        if binding.matches(&key) {
            return Some(binding.message);
        }
    }
    None
}

// ── Hint lines ───────────────────────────────────────────────────────────────

/// Build a hint line from the binding table for a given context.
pub fn hint_line(ctx: InputContext, theme: &Theme) -> Line<'static> {
    Line::from(hint_spans(bindings_for(ctx), theme))
}

/// Build a hint line for the DirtyCheckout overlay.
pub fn dirty_checkout_hint_line(has_conflicts: bool, theme: &Theme) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    if has_conflicts {
        lines.push(Line::from(Span::styled(
            " Stash unavailable (unmerged files)",
            Style::default().fg(theme.fg_dim),
        )));
    } else {
        lines.push(Line::from(hint_entry_spans(&DIRTY_CHECKOUT_STASH, theme)));
    }

    for binding in DIRTY_CHECKOUT_BASE {
        if binding.show_in_hint {
            lines.push(Line::from(hint_entry_spans(binding, theme)));
        }
    }

    lines
}

/// Format a single binding as " [key] label" spans (for vertical hint lists like DirtyCheckout/Confirm).
fn hint_entry_spans(binding: &Binding, theme: &Theme) -> Vec<Span<'static>> {
    vec![
        Span::styled(
            format!(" [{}] ", display_keys(binding.keys)),
            Style::default().fg(theme.yellow),
        ),
        Span::styled(binding.label.to_string(), Style::default().fg(theme.fg_dim)),
    ]
}

/// Build hint spans from a binding slice, merging entries with the same label.
fn hint_spans(bindings: &[Binding], theme: &Theme) -> Vec<Span<'static>> {
    // Collect (key_displays, label) merging by label
    let mut entries: Vec<(Vec<String>, &str)> = Vec::new();
    for b in bindings {
        if !b.show_in_hint {
            continue;
        }
        let keys_str = display_keys(b.keys);
        if let Some(existing) = entries.iter_mut().find(|(_, l)| *l == b.label) {
            existing.0.push(keys_str);
        } else {
            entries.push((vec![keys_str], b.label));
        }
    }

    let mut spans = Vec::new();
    for (key_strs, label) in &entries {
        if !spans.is_empty() {
            spans.push(Span::styled("  ", Style::default()));
        }
        spans.push(Span::styled(
            merge_key_displays(key_strs),
            Style::default().fg(theme.yellow),
        ));
        spans.push(Span::styled(
            format!(":{label}"),
            Style::default().fg(theme.fg_dim),
        ));
    }
    spans
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, modifiers)
    }

    // ── Global keys ──

    #[test]
    fn test_ctrl_c_quits_in_all_contexts() {
        let ctrl_c = key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(matches!(
            resolve(InputContext::FileList, ctrl_c),
            Some(Message::Quit)
        ));
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, ctrl_c),
            Some(Message::Quit)
        ));
        assert!(matches!(
            resolve(InputContext::DiffLineNav, ctrl_c),
            Some(Message::Quit)
        ));
    }

    #[test]
    fn test_ctrl_c_does_not_quit_overlays() {
        let ctrl_c = key_mod(KeyCode::Char('c'), KeyModifiers::CONTROL);
        // Confirm overlay: Ctrl+C is not bound
        assert!(resolve(InputContext::Confirm, ctrl_c).is_none());
        // CommitInput: Ctrl+C → CloseOverlay
        assert!(matches!(
            resolve(InputContext::CommitInput, ctrl_c),
            Some(Message::CloseOverlay)
        ));
    }

    #[test]
    fn test_q_quits_in_filelist_and_hunknav() {
        let q = key(KeyCode::Char('q'));
        assert!(matches!(
            resolve(InputContext::FileList, q),
            Some(Message::Quit)
        ));
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, q),
            Some(Message::Quit)
        ));
    }

    #[test]
    fn test_q_exits_line_mode() {
        let q = key(KeyCode::Char('q'));
        assert!(matches!(
            resolve(InputContext::DiffLineNav, q),
            Some(Message::ExitLineMode)
        ));
    }

    #[test]
    fn test_q_closes_conflict() {
        let q = key(KeyCode::Char('q'));
        assert!(matches!(
            resolve(InputContext::ConflictNav, q),
            Some(Message::CloseConflict)
        ));
    }

    #[test]
    fn test_filelist_arrow_down_moves_down() {
        assert!(matches!(
            resolve(InputContext::FileList, key(KeyCode::Down)),
            Some(Message::MoveDown)
        ));
    }

    #[test]
    fn test_filelist_arrow_up_moves_up() {
        assert!(matches!(
            resolve(InputContext::FileList, key(KeyCode::Up)),
            Some(Message::MoveUp)
        ));
    }

    #[test]
    fn test_tab_is_not_bound() {
        assert!(resolve(InputContext::FileList, key(KeyCode::Tab)).is_none());
        assert!(resolve(InputContext::DiffHunkNav, key(KeyCode::Tab)).is_none());
    }

    #[test]
    fn test_space_opens_which_key() {
        assert!(matches!(
            resolve(InputContext::FileList, key(KeyCode::Char(' '))),
            Some(Message::OpenWhichKey)
        ));
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, key(KeyCode::Char(' '))),
            Some(Message::OpenWhichKey)
        ));
    }

    // ── Diff arrow keys: plain=hunk, shift=scroll ──

    #[test]
    fn test_diff_hunk_arrow_down_next_hunk() {
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, key(KeyCode::Down)),
            Some(Message::NextHunk)
        ));
    }

    #[test]
    fn test_diff_hunk_arrow_up_prev_hunk() {
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, key(KeyCode::Up)),
            Some(Message::PrevHunk)
        ));
    }

    #[test]
    fn test_diff_hunk_shift_down_scrolls() {
        let shift_down = key_mod(KeyCode::Down, KeyModifiers::SHIFT);
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, shift_down),
            Some(Message::MoveDown)
        ));
    }

    #[test]
    fn test_diff_hunk_shift_up_scrolls() {
        let shift_up = key_mod(KeyCode::Up, KeyModifiers::SHIFT);
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, shift_up),
            Some(Message::MoveUp)
        ));
    }

    #[test]
    fn test_diff_line_arrow_down_moves_down() {
        assert!(matches!(
            resolve(InputContext::DiffLineNav, key(KeyCode::Down)),
            Some(Message::MoveDown)
        ));
    }

    #[test]
    fn test_diff_line_arrow_up_moves_up() {
        assert!(matches!(
            resolve(InputContext::DiffLineNav, key(KeyCode::Up)),
            Some(Message::MoveUp)
        ));
    }

    #[test]
    fn test_diff_line_shift_down_next_hunk() {
        let shift_down = key_mod(KeyCode::Down, KeyModifiers::SHIFT);
        assert!(matches!(
            resolve(InputContext::DiffLineNav, shift_down),
            Some(Message::NextHunk)
        ));
    }

    #[test]
    fn test_diff_line_shift_up_prev_hunk() {
        let shift_up = key_mod(KeyCode::Up, KeyModifiers::SHIFT);
        assert!(matches!(
            resolve(InputContext::DiffLineNav, shift_up),
            Some(Message::PrevHunk)
        ));
    }

    // ── FileList context ──

    #[test]
    fn test_filelist_enter_switches_panel() {
        assert!(matches!(
            resolve(InputContext::FileList, key(KeyCode::Enter)),
            Some(Message::SwitchPanel)
        ));
    }

    #[test]
    fn test_filelist_right_switches_panel() {
        assert!(matches!(
            resolve(InputContext::FileList, key(KeyCode::Right)),
            Some(Message::SwitchPanel)
        ));
    }

    #[test]
    fn test_filelist_slash_starts_filter() {
        assert!(matches!(
            resolve(InputContext::FileList, key(KeyCode::Char('/'))),
            Some(Message::StartFilter)
        ));
    }

    #[test]
    fn test_filelist_unknown_key_returns_none() {
        assert!(resolve(InputContext::FileList, key(KeyCode::Char('z'))).is_none());
    }

    // ── DiffHunkNav context ──

    #[test]
    fn test_hunknav_left_switches_panel() {
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, key(KeyCode::Left)),
            Some(Message::SwitchPanel)
        ));
    }

    #[test]
    fn test_hunknav_enter_enters_line_mode() {
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, key(KeyCode::Enter)),
            Some(Message::EnterLineMode)
        ));
    }

    #[test]
    fn test_hunknav_right_enters_line_mode() {
        assert!(matches!(
            resolve(InputContext::DiffHunkNav, key(KeyCode::Right)),
            Some(Message::EnterLineMode)
        ));
    }

    // ── DiffLineNav context ──

    #[test]
    fn test_linenav_enter_toggles_line() {
        assert!(matches!(
            resolve(InputContext::DiffLineNav, key(KeyCode::Enter)),
            Some(Message::ToggleLine)
        ));
    }

    #[test]
    fn test_linenav_right_toggles_line() {
        assert!(matches!(
            resolve(InputContext::DiffLineNav, key(KeyCode::Right)),
            Some(Message::ToggleLine)
        ));
    }

    #[test]
    fn test_linenav_esc_exits_line_mode() {
        assert!(matches!(
            resolve(InputContext::DiffLineNav, key(KeyCode::Esc)),
            Some(Message::ExitLineMode)
        ));
    }

    #[test]
    fn test_linenav_left_exits_line_mode() {
        assert!(matches!(
            resolve(InputContext::DiffLineNav, key(KeyCode::Left)),
            Some(Message::ExitLineMode)
        ));
    }

    // ── ConflictNav context ──

    #[test]
    fn test_conflictnav_left_closes() {
        assert!(matches!(
            resolve(InputContext::ConflictNav, key(KeyCode::Left)),
            Some(Message::CloseConflict)
        ));
    }

    #[test]
    fn test_conflictnav_esc_closes() {
        assert!(matches!(
            resolve(InputContext::ConflictNav, key(KeyCode::Esc)),
            Some(Message::CloseConflict)
        ));
    }

    #[test]
    fn test_conflictnav_space_opens_which_key() {
        assert!(matches!(
            resolve(InputContext::ConflictNav, key(KeyCode::Char(' '))),
            Some(Message::OpenWhichKey)
        ));
    }

    // ── Overlay contexts ──

    #[test]
    fn test_confirm_yes() {
        assert!(matches!(
            resolve(InputContext::Confirm, key(KeyCode::Char('y'))),
            Some(Message::ConfirmAction)
        ));
        assert!(matches!(
            resolve(InputContext::Confirm, key(KeyCode::Enter)),
            Some(Message::ConfirmAction)
        ));
    }

    #[test]
    fn test_confirm_no() {
        assert!(matches!(
            resolve(InputContext::Confirm, key(KeyCode::Char('n'))),
            Some(Message::CloseOverlay)
        ));
        assert!(matches!(
            resolve(InputContext::Confirm, key(KeyCode::Esc)),
            Some(Message::CloseOverlay)
        ));
    }

    #[test]
    fn test_commit_input_ctrl_s_confirms() {
        let ctrl_s = key_mod(KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert!(matches!(
            resolve(InputContext::CommitInput, ctrl_s),
            Some(Message::ConfirmCommit)
        ));
    }

    #[test]
    fn test_commit_input_text_keys_return_none() {
        assert!(resolve(InputContext::CommitInput, key(KeyCode::Char('a'))).is_none());
        assert!(resolve(InputContext::CommitInput, key(KeyCode::Enter)).is_none());
    }

    #[test]
    fn test_git_log_bindings() {
        assert!(matches!(
            resolve(InputContext::GitLog, key(KeyCode::Char('y'))),
            Some(Message::YankToClipboard)
        ));
        assert!(matches!(
            resolve(InputContext::GitLog, key(KeyCode::Enter)),
            Some(Message::ViewCommitDetail)
        ));
        assert!(matches!(
            resolve(InputContext::GitLog, key(KeyCode::Char('r'))),
            Some(Message::StartRebase)
        ));
    }

    #[test]
    fn test_stash_list_bindings() {
        assert!(matches!(
            resolve(InputContext::StashList, key(KeyCode::Char('p'))),
            Some(Message::StashPop)
        ));
        assert!(matches!(
            resolve(InputContext::StashList, key(KeyCode::Char('a'))),
            Some(Message::StashApply)
        ));
    }

    #[test]
    fn test_branch_list_bindings() {
        assert!(matches!(
            resolve(InputContext::BranchList, key(KeyCode::Enter)),
            Some(Message::CheckoutBranch)
        ));
        assert!(matches!(
            resolve(InputContext::BranchList, key(KeyCode::Char('n'))),
            Some(Message::StartCreateBranch)
        ));
    }

    #[test]
    fn test_rebase_space_cycles_action() {
        assert!(matches!(
            resolve(InputContext::Rebase, key(KeyCode::Char(' '))),
            Some(Message::RebaseCycleAction)
        ));
    }

    #[test]
    fn test_commit_detail_bindings() {
        let ctrl_down = key_mod(KeyCode::Down, KeyModifiers::CONTROL);
        assert!(matches!(
            resolve(InputContext::CommitDetail, ctrl_down),
            Some(Message::NextCommitDetail)
        ));
        assert!(matches!(
            resolve(InputContext::CommitDetail, key(KeyCode::Down)),
            Some(Message::NextHunkCommitDetail)
        ));
    }

    #[test]
    fn test_dirty_checkout_stash() {
        assert!(matches!(
            resolve_dirty_checkout(key(KeyCode::Char('s')), false),
            Some(Message::DirtyCheckoutStash)
        ));
        // With conflicts, 's' is not available
        assert!(resolve_dirty_checkout(key(KeyCode::Char('s')), true).is_none());
    }

    #[test]
    fn test_dirty_checkout_discard() {
        assert!(matches!(
            resolve_dirty_checkout(key(KeyCode::Char('d')), false),
            Some(Message::DirtyCheckoutDiscard)
        ));
        assert!(matches!(
            resolve_dirty_checkout(key(KeyCode::Char('d')), true),
            Some(Message::DirtyCheckoutDiscard)
        ));
    }
}
