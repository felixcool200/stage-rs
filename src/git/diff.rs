use color_eyre::{eyre::eyre, Result};
use git2::Repository;
use similar::{ChangeTag, TextDiff};
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
pub struct LinePair {
    pub left: Option<usize>,
    pub right: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct Hunk {
    pub display_start: usize,
    pub display_end: usize, // exclusive
    pub header: String,     // e.g. "@@ -10,5 +12,7 @@"
}

/// Get the index (staged) content and working tree content for a file.
pub fn get_diff_content(repo: &Repository, path: &str) -> Result<(String, String)> {
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

/// Compute side-by-side diff lines, line mapping, and hunks.
pub fn compute_diff(
    old: &str,
    new: &str,
) -> (Vec<DiffLine>, Vec<DiffLine>, Vec<LinePair>, Vec<Hunk>) {
    let diff = TextDiff::from_lines(old, new);
    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();
    let mut mapping = Vec::new();
    let mut hunks = Vec::new();

    let mut current_hunk_start: Option<usize> = None;
    let mut hunk_index: usize = 0;
    // Track original line numbers for hunk headers
    let mut old_line: usize = 0;
    let mut new_line: usize = 0;
    let mut hunk_old_start: usize = 0;
    let mut hunk_new_start: usize = 0;
    let mut hunk_old_count: usize = 0;
    let mut hunk_new_count: usize = 0;

    for change in diff.iter_all_changes() {
        let text = change.value().trim_end_matches('\n').to_string();
        let display_row = left_lines.len();

        match change.tag() {
            ChangeTag::Equal => {
                // Close any open hunk
                if let Some(start) = current_hunk_start.take() {
                    hunks.push(Hunk {
                        display_start: start,
                        display_end: display_row,
                        header: format!(
                            "@@ -{},{} +{},{} @@",
                            hunk_old_start + 1,
                            hunk_old_count,
                            hunk_new_start + 1,
                            hunk_new_count
                        ),
                    });
                    hunk_index += 1;
                }

                let li = left_lines.len();
                let ri = right_lines.len();
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
                mapping.push(LinePair {
                    left: Some(li),
                    right: Some(ri),
                });
                old_line += 1;
                new_line += 1;
            }
            ChangeTag::Delete => {
                if current_hunk_start.is_none() {
                    current_hunk_start = Some(display_row);
                    hunk_old_start = old_line;
                    hunk_new_start = new_line;
                    hunk_old_count = 0;
                    hunk_new_count = 0;
                }
                hunk_old_count += 1;

                let li = left_lines.len();
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
                mapping.push(LinePair {
                    left: Some(li),
                    right: None,
                });
                old_line += 1;
            }
            ChangeTag::Insert => {
                if current_hunk_start.is_none() {
                    current_hunk_start = Some(display_row);
                    hunk_old_start = old_line;
                    hunk_new_start = new_line;
                    hunk_old_count = 0;
                    hunk_new_count = 0;
                }
                hunk_new_count += 1;

                let ri = right_lines.len();
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
                mapping.push(LinePair {
                    left: None,
                    right: Some(ri),
                });
                new_line += 1;
            }
        }
    }

    // Close final hunk if open
    if let Some(start) = current_hunk_start {
        hunks.push(Hunk {
            display_start: start,
            display_end: left_lines.len(),
            header: format!(
                "@@ -{},{} +{},{} @@",
                hunk_old_start + 1,
                hunk_old_count,
                hunk_new_start + 1,
                hunk_new_count
            ),
        });
    }

    (left_lines, right_lines, mapping, hunks)
}

/// Apply only the specified hunk to the index content, producing a new file.
/// `selected_hunk` is the index of the hunk to stage.
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
                    // Not staging this hunk — keep the old line
                    result.push(change.value().to_string());
                }
                // If staging: skip old line (it will be replaced by inserts)
            }
            ChangeTag::Insert => {
                in_change = true;
                if current_hunk == selected_hunk {
                    // Staging this hunk — include the new line
                    result.push(change.value().to_string());
                }
                // If not staging: skip new line (keep old version)
            }
        }
    }

    result.join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_hunk_stages_only_selected() {
        let old = "line1\nline2\nline3\nline4\nline5\n";
        let new = "line1\nchanged2\nline3\nline4\nadded\nline5\n";
        // Two hunks: hunk 0 = line2→changed2, hunk 1 = added line before line5

        // Stage only hunk 0: line2 becomes changed2, but "added" is NOT included
        let result0 = apply_hunk(old, new, 0);
        assert_eq!(result0, "line1\nchanged2\nline3\nline4\nline5\n");

        // Stage only hunk 1: line2 stays, but "added" IS included
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

        // Stage the deletion
        let result = apply_hunk(old, new, 0);
        assert_eq!(result, "a\nb\n");
    }

    #[test]
    fn test_compute_diff_hunk_count() {
        let old = "a\nb\nc\nd\ne\n";
        let new = "a\nB\nc\nd\nE\n";

        let (_, _, _, hunks) = compute_diff(old, new);
        assert_eq!(hunks.len(), 2);
    }
}
