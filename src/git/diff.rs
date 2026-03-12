use color_eyre::{eyre::eyre, Result};
use git2::Repository;
use similar::{ChangeTag, TextDiff};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub kind: DiffLineKind,
    pub hunk_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLineKind {
    Equal,
    Added,
    Removed,
    Spacer,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub display_start: usize,
    pub display_end: usize, // exclusive
}

/// Get diff content for a file.
/// For unstaged/untracked files: index (or HEAD) vs working tree.
/// For staged files: HEAD vs index.
pub fn get_diff_content(repo: &Repository, path: &str, staged: bool) -> Result<(String, String)> {
    if staged {
        let head_content = get_head_content(repo, path).unwrap_or_default();
        let index_content = get_index_content(repo, path).unwrap_or_default();
        Ok((head_content, index_content))
    } else {
        let workdir = repo.workdir().ok_or_else(|| eyre!("bare repo"))?;
        let full_path = workdir.join(path);

        let workdir_content = if full_path.exists() {
            std::fs::read_to_string(&full_path)?
        } else {
            String::new()
        };

        let index_content = get_index_content(repo, path)
            .or_else(|_| get_head_content(repo, path))
            .unwrap_or_default();

        Ok((index_content, workdir_content))
    }
}

fn get_index_content(repo: &Repository, path: &str) -> Result<String> {
    let index = repo.index()?;
    let entry = index
        .get_path(Path::new(path), 0)
        .ok_or_else(|| eyre!("file not in index"))?;
    let blob = repo.find_blob(entry.id)?;
    Ok(String::from_utf8_lossy(blob.content()).to_string())
}

fn get_head_content(repo: &Repository, path: &str) -> Result<String> {
    let head = repo.head()?;
    let tree = head.peel_to_tree()?;
    let entry = tree
        .get_path(Path::new(path))
        .map_err(|e| eyre!("not in HEAD: {e}"))?;
    let blob = repo
        .find_blob(entry.id())
        .map_err(|e| eyre!("not a blob: {e}"))?;
    Ok(String::from_utf8_lossy(blob.content()).to_string())
}

/// Compute side-by-side diff lines and hunks.
pub fn compute_diff(
    old: &str,
    new: &str,
) -> (Vec<DiffLine>, Vec<DiffLine>, Vec<Hunk>) {
    let diff = TextDiff::from_lines(old, new);
    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();
    let mut hunks = Vec::new();

    let mut current_hunk_start: Option<usize> = None;
    let mut hunk_index: usize = 0;

    for change in diff.iter_all_changes() {
        let text = change.value().trim_end_matches('\n').to_string();
        let display_row = left_lines.len();

        match change.tag() {
            ChangeTag::Equal => {
                if let Some(start) = current_hunk_start.take() {
                    hunks.push(Hunk {
                        display_start: start,
                        display_end: display_row,
                    });
                    hunk_index += 1;
                }

                left_lines.push(DiffLine {
                    content: text.clone(),
                    kind: DiffLineKind::Equal,
                    hunk_index: None,
                });
                right_lines.push(DiffLine {
                    content: text,
                    kind: DiffLineKind::Equal,
                    hunk_index: None,
                });
            }
            ChangeTag::Delete => {
                if current_hunk_start.is_none() {
                    current_hunk_start = Some(display_row);
                }

                left_lines.push(DiffLine {
                    content: text,
                    kind: DiffLineKind::Removed,
                    hunk_index: Some(hunk_index),
                });
                right_lines.push(DiffLine {
                    content: String::new(),
                    kind: DiffLineKind::Spacer,
                    hunk_index: Some(hunk_index),
                });
            }
            ChangeTag::Insert => {
                if current_hunk_start.is_none() {
                    current_hunk_start = Some(display_row);
                }

                left_lines.push(DiffLine {
                    content: String::new(),
                    kind: DiffLineKind::Spacer,
                    hunk_index: Some(hunk_index),
                });
                right_lines.push(DiffLine {
                    content: text,
                    kind: DiffLineKind::Added,
                    hunk_index: Some(hunk_index),
                });
            }
        }
    }

    if let Some(start) = current_hunk_start {
        hunks.push(Hunk {
            display_start: start,
            display_end: left_lines.len(),
        });
    }

    (left_lines, right_lines, hunks)
}

/// Apply only the specified hunk to the index content, producing a new file.
pub fn apply_hunk(old: &str, new: &str, selected_hunk: usize) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut result = Vec::new();
    let mut current_hunk: usize = 0;
    let mut in_change = false;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                if in_change {
                    current_hunk += 1;
                    in_change = false;
                }
                result.push(change.value().to_string());
            }
            ChangeTag::Delete => {
                in_change = true;
                if current_hunk != selected_hunk {
                    result.push(change.value().to_string());
                }
            }
            ChangeTag::Insert => {
                in_change = true;
                if current_hunk == selected_hunk {
                    result.push(change.value().to_string());
                }
            }
        }
    }

    result.join("")
}

/// Apply only the specified display rows to the index content.
/// `selected_rows` contains display row indices of changed lines to stage.
/// Display rows map 1:1 with diff changes (same order as compute_diff output).
pub fn apply_lines(old: &str, new: &str, selected_rows: &BTreeSet<usize>) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut result = Vec::new();
    let mut display_row: usize = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                result.push(change.value().to_string());
                display_row += 1;
            }
            ChangeTag::Delete => {
                if !selected_rows.contains(&display_row) {
                    // Not staging this deletion — keep the old line
                    result.push(change.value().to_string());
                }
                display_row += 1;
            }
            ChangeTag::Insert => {
                if selected_rows.contains(&display_row) {
                    // Staging this addition — include the new line
                    result.push(change.value().to_string());
                }
                display_row += 1;
            }
        }
    }

    result.join("")
}

/// Get all display rows that are changed lines within a hunk.
pub fn changed_rows_in_hunk(hunk: &Hunk, left_lines: &[DiffLine]) -> Vec<usize> {
    (hunk.display_start..hunk.display_end)
        .filter(|&i| left_lines[i].hunk_index.is_some() && left_lines[i].kind != DiffLineKind::Spacer
            || (i < left_lines.len() && left_lines[i].kind == DiffLineKind::Spacer
                && left_lines[i].hunk_index.is_some()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_hunk_stages_only_selected() {
        let old = "line1\nline2\nline3\nline4\nline5\n";
        let new = "line1\nchanged2\nline3\nline4\nadded\nline5\n";

        let result0 = apply_hunk(old, new, 0);
        assert_eq!(result0, "line1\nchanged2\nline3\nline4\nline5\n");

        let result1 = apply_hunk(old, new, 1);
        assert_eq!(result1, "line1\nline2\nline3\nline4\nadded\nline5\n");
    }

    #[test]
    fn test_apply_hunk_pure_addition() {
        let old = "a\nb\n";
        let new = "a\nnew\nb\n";

        let result = apply_hunk(old, new, 0);
        assert_eq!(result, "a\nnew\nb\n");
    }

    #[test]
    fn test_apply_hunk_pure_deletion() {
        let old = "a\ndelete_me\nb\n";
        let new = "a\nb\n";

        let result = apply_hunk(old, new, 0);
        assert_eq!(result, "a\nb\n");
    }

    #[test]
    fn test_compute_diff_hunk_count() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nB\nc\nd\nE\n";

        let (_, _, hunks) = compute_diff(old, new);
        assert_eq!(hunks.len(), 2);
    }

    #[test]
    fn test_apply_lines_selective() {
        // old: line1, line2, line3, line4, line5
        // new: line1, changed2, line3, changed4, line5
        // Display rows:
        //   0: Equal "line1"
        //   1: Delete "line2"   (hunk 0)
        //   2: Insert "changed2" (hunk 0)
        //   3: Equal "line3"
        //   4: Delete "line4"   (hunk 1)
        //   5: Insert "changed4" (hunk 1)
        //   6: Equal "line5"
        let old = "line1\nline2\nline3\nline4\nline5\n";
        let new = "line1\nchanged2\nline3\nchanged4\nline5\n";

        // Stage only the first change (rows 1 and 2)
        let mut selected = BTreeSet::new();
        selected.insert(1); // delete line2
        selected.insert(2); // insert changed2
        let result = apply_lines(old, new, &selected);
        assert_eq!(result, "line1\nchanged2\nline3\nline4\nline5\n");

        // Stage only the second change (rows 4 and 5)
        let mut selected = BTreeSet::new();
        selected.insert(4); // delete line4
        selected.insert(5); // insert changed4
        let result = apply_lines(old, new, &selected);
        assert_eq!(result, "line1\nline2\nline3\nchanged4\nline5\n");
    }

    #[test]
    fn test_apply_lines_stage_only_addition() {
        // Stage only the insert, not the delete in a replace
        let old = "a\nold\nb\n";
        let new = "a\nnew\nb\n";
        // row 0: Equal "a"
        // row 1: Delete "old"
        // row 2: Insert "new"
        // row 3: Equal "b"

        // Stage only the addition (keep old line AND add new)
        let mut selected = BTreeSet::new();
        selected.insert(2);
        let result = apply_lines(old, new, &selected);
        assert_eq!(result, "a\nold\nnew\nb\n");
    }

    #[test]
    fn test_apply_lines_stage_only_deletion() {
        let old = "a\nold\nb\n";
        let new = "a\nnew\nb\n";

        // Stage only the deletion (remove old, don't add new)
        let mut selected = BTreeSet::new();
        selected.insert(1);
        let result = apply_lines(old, new, &selected);
        assert_eq!(result, "a\nb\n");
    }

    #[test]
    fn test_compute_diff_empty_strings() {
        let (left, right, hunks) = compute_diff("", "");
        assert!(left.is_empty());
        assert!(right.is_empty());
        assert!(hunks.is_empty());
    }

    #[test]
    fn test_compute_diff_identical_input() {
        let text = "line1\nline2\nline3\n";
        let (left, right, hunks) = compute_diff(text, text);
        assert!(hunks.is_empty());
        assert!(left.iter().all(|l| l.kind == DiffLineKind::Equal));
        assert!(right.iter().all(|l| l.kind == DiffLineKind::Equal));
    }

    #[test]
    fn test_compute_diff_all_added() {
        let (left, right, hunks) = compute_diff("", "a\nb\n");
        assert_eq!(hunks.len(), 1);
        assert!(left.iter().all(|l| l.kind == DiffLineKind::Spacer));
        assert!(right.iter().all(|l| l.kind == DiffLineKind::Added));
    }

    #[test]
    fn test_compute_diff_all_removed() {
        let (left, right, hunks) = compute_diff("a\nb\n", "");
        assert_eq!(hunks.len(), 1);
        assert!(left.iter().all(|l| l.kind == DiffLineKind::Removed));
        assert!(right.iter().all(|l| l.kind == DiffLineKind::Spacer));
    }

    #[test]
    fn test_compute_diff_left_right_same_length() {
        let old = "a\nb\nc\n";
        let new = "a\nX\nc\n";
        let (left, right, _) = compute_diff(old, new);
        assert_eq!(left.len(), right.len());
    }

    #[test]
    fn test_compute_diff_hunk_indices_consistent() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nB\nc\nd\nE\n";
        let (left, right, hunks) = compute_diff(old, new);

        // All changed lines should have hunk_index set
        for line in &left {
            if line.kind != DiffLineKind::Equal {
                assert!(line.hunk_index.is_some());
            }
        }
        for line in &right {
            if line.kind != DiffLineKind::Equal {
                assert!(line.hunk_index.is_some());
            }
        }

        // Hunk ranges should be valid
        for hunk in &hunks {
            assert!(hunk.display_start < hunk.display_end);
            assert!(hunk.display_end <= left.len());
        }
    }

    #[test]
    fn test_changed_rows_in_hunk() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nB\nc\nd\nE\n";
        let (left, _, hunks) = compute_diff(old, new);

        assert_eq!(hunks.len(), 2);
        let rows0 = changed_rows_in_hunk(&hunks[0], &left);
        let rows1 = changed_rows_in_hunk(&hunks[1], &left);

        // Each hunk should have changed rows within its range
        for &r in &rows0 {
            assert!(r >= hunks[0].display_start && r < hunks[0].display_end);
        }
        for &r in &rows1 {
            assert!(r >= hunks[1].display_start && r < hunks[1].display_end);
        }

        // Both hunks should have non-empty changed rows
        assert!(!rows0.is_empty());
        assert!(!rows1.is_empty());
    }

    #[test]
    fn test_apply_hunk_no_change_when_wrong_index() {
        let old = "a\nb\nc\n";
        let new = "a\nB\nc\n";

        // Selecting a hunk index that doesn't exist should keep old content
        let result = apply_hunk(old, new, 99);
        assert_eq!(result, old);
    }

    #[test]
    fn test_apply_lines_empty_selection() {
        let old = "a\nold\nb\n";
        let new = "a\nnew\nb\n";

        // No rows selected = keep old content
        let selected = BTreeSet::new();
        let result = apply_lines(old, new, &selected);
        assert_eq!(result, old);
    }
}
