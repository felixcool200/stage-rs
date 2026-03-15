use crate::app::Message;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Context passed to the keymap resolver so it knows which mode we're in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputContext {
    FileList,
    DiffHunkNav,
    DiffLineNav,
    ConflictNav,
}

/// Resolve a key event to a message given the current context.
pub fn resolve(ctx: InputContext, key: KeyEvent) -> Option<Message> {
    if key.modifiers == KeyModifiers::CONTROL && key.code == KeyCode::Char('c') {
        return Some(Message::Quit);
    }

    if key.code == KeyCode::Char(' ') {
        return Some(Message::OpenWhichKey);
    }

    // Context-specific navigation
    match ctx {
        InputContext::FileList => match key.code {
            KeyCode::Char('q') => Some(Message::Quit),
            KeyCode::Down => Some(Message::MoveDown),
            KeyCode::Up => Some(Message::MoveUp),
            KeyCode::Enter | KeyCode::Right => Some(Message::SwitchPanel),
            KeyCode::Char('/') => Some(Message::StartFilter),
            _ => None,
        },
        InputContext::DiffHunkNav => match (key.modifiers, key.code) {
            (_, KeyCode::Char('q')) => Some(Message::Quit),
            (KeyModifiers::SHIFT, KeyCode::Down) => Some(Message::MoveDown),
            (KeyModifiers::SHIFT, KeyCode::Up) => Some(Message::MoveUp),
            (_, KeyCode::Down) => Some(Message::NextHunk),
            (_, KeyCode::Up) => Some(Message::PrevHunk),
            (_, KeyCode::Left) => Some(Message::SwitchPanel),
            (_, KeyCode::Enter) | (_, KeyCode::Right) => Some(Message::EnterLineMode),
            _ => None,
        },
        InputContext::DiffLineNav => match (key.modifiers, key.code) {
            (KeyModifiers::SHIFT, KeyCode::Down) => Some(Message::NextHunk),
            (KeyModifiers::SHIFT, KeyCode::Up) => Some(Message::PrevHunk),
            (_, KeyCode::Down) => Some(Message::MoveDown),
            (_, KeyCode::Up) => Some(Message::MoveUp),
            (_, KeyCode::Enter) | (_, KeyCode::Right) => Some(Message::ToggleLine),
            (_, KeyCode::Esc) | (_, KeyCode::Left) | (_, KeyCode::Char('q')) => {
                Some(Message::ExitLineMode)
            }
            _ => None,
        },
        InputContext::ConflictNav => match key.code {
            KeyCode::Down => Some(Message::MoveDown),
            KeyCode::Up => Some(Message::MoveUp),
            KeyCode::Left | KeyCode::Esc | KeyCode::Char('q') => Some(Message::CloseConflict),
            _ => None,
        },
    }
}

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
}
