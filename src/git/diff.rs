use color_eyre::{eyre::eyre, Result};
use git2::Repository;
use similar::{ChangeTag, TextDiff};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub content: String,
    pub kind: DiffLineKind,
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

/// Compute side-by-side diff lines and line mapping.
pub fn compute_diff(
    old: &str,
    new: &str,
) -> (Vec<DiffLine>, Vec<DiffLine>, Vec<LinePair>) {
    let diff = TextDiff::from_lines(old, new);
    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();
    let mut mapping = Vec::new();

    for change in diff.iter_all_changes() {
        let text = change.value().trim_end_matches('\n').to_string();
        match change.tag() {
            ChangeTag::Equal => {
                let li = left_lines.len();
                let ri = right_lines.len();
                left_lines.push(DiffLine {
                    content: text.clone(),
                    kind: DiffLineKind::Equal,
                });
                right_lines.push(DiffLine {
                    content: text,
                    kind: DiffLineKind::Equal,
                });
                mapping.push(LinePair {
                    left: Some(li),
                    right: Some(ri),
                });
            }
            ChangeTag::Delete => {
                let li = left_lines.len();
                left_lines.push(DiffLine {
                    content: text,
                    kind: DiffLineKind::Removed,
                });
                right_lines.push(DiffLine {
                    content: String::new(),
                    kind: DiffLineKind::Spacer,
                });
                mapping.push(LinePair {
                    left: Some(li),
                    right: None,
                });
            }
            ChangeTag::Insert => {
                let ri = right_lines.len();
                left_lines.push(DiffLine {
                    content: String::new(),
                    kind: DiffLineKind::Spacer,
                });
                right_lines.push(DiffLine {
                    content: text,
                    kind: DiffLineKind::Added,
                });
                mapping.push(LinePair {
                    left: None,
                    right: Some(ri),
                });
            }
        }
    }

    (left_lines, right_lines, mapping)
}
