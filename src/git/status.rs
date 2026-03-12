use color_eyre::Result;
use git2::{DiffOptions, Repository, Status, StatusOptions};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub status: FileStatus,
    pub insertions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Staged(ChangeKind),
    Unstaged(ChangeKind),
    Conflict,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Renamed,
}

impl FileStatus {
    pub fn sort_key(&self) -> u8 {
        match self {
            FileStatus::Conflict => 0,
            FileStatus::Unstaged(_) => 1,
            FileStatus::Untracked => 2,
            FileStatus::Staged(_) => 3,
        }
    }

    pub fn short_label(&self) -> &str {
        match self {
            FileStatus::Staged(k) | FileStatus::Unstaged(k) => match k {
                ChangeKind::Modified => "M",
                ChangeKind::Added => "A",
                ChangeKind::Deleted => "D",
                ChangeKind::Renamed => "R",
            },
            FileStatus::Conflict => "C",
            FileStatus::Untracked => "?",
        }
    }

    pub fn section_name(&self) -> &str {
        match self {
            FileStatus::Conflict => "Merge Conflicts",
            FileStatus::Unstaged(_) => "Changes",
            FileStatus::Untracked => "Untracked",
            FileStatus::Staged(_) => "Staged Changes",
        }
    }
}

pub fn get_file_statuses(repo: &Repository) -> Result<Vec<FileEntry>> {
    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .include_unmodified(false);

    let statuses = repo.statuses(Some(&mut opts))?;
    let mut entries = Vec::new();

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let s = entry.status();

        // Check for conflicts first
        if s.contains(Status::CONFLICTED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Conflict,
                insertions: 0,
                deletions: 0,
            });
            continue;
        }

        // Staged changes (index vs HEAD)
        if s.contains(Status::INDEX_NEW) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Staged(ChangeKind::Added), insertions: 0, deletions: 0 });
        } else if s.contains(Status::INDEX_MODIFIED) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Staged(ChangeKind::Modified), insertions: 0, deletions: 0 });
        } else if s.contains(Status::INDEX_DELETED) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Staged(ChangeKind::Deleted), insertions: 0, deletions: 0 });
        } else if s.contains(Status::INDEX_RENAMED) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Staged(ChangeKind::Renamed), insertions: 0, deletions: 0 });
        }

        // Unstaged changes (workdir vs index)
        if s.contains(Status::WT_MODIFIED) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Unstaged(ChangeKind::Modified), insertions: 0, deletions: 0 });
        } else if s.contains(Status::WT_DELETED) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Unstaged(ChangeKind::Deleted), insertions: 0, deletions: 0 });
        } else if s.contains(Status::WT_RENAMED) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Unstaged(ChangeKind::Renamed), insertions: 0, deletions: 0 });
        } else if s.contains(Status::WT_NEW) {
            entries.push(FileEntry { path: path.clone(), status: FileStatus::Untracked, insertions: 0, deletions: 0 });
        }
    }

    entries.sort_by(|a, b| {
        a.status
            .sort_key()
            .cmp(&b.status.sort_key())
            .then(a.path.cmp(&b.path))
    });

    // Compute per-file diff stats
    let staged_stats = diff_stats_index_to_head(repo);
    let unstaged_stats = diff_stats_workdir_to_index(repo);

    for entry in &mut entries {
        let stats_map = match &entry.status {
            FileStatus::Staged(_) => &staged_stats,
            FileStatus::Unstaged(_) | FileStatus::Untracked => &unstaged_stats,
            FileStatus::Conflict => &unstaged_stats,
        };
        if let Some((ins, del)) = stats_map.get(entry.path.as_str()) {
            entry.insertions = *ins;
            entry.deletions = *del;
        }
    }

    Ok(entries)
}

fn diff_stats_index_to_head(repo: &Repository) -> HashMap<String, (usize, usize)> {
    let mut map = HashMap::new();
    let head_tree = repo.head().ok()
        .and_then(|h| h.peel_to_tree().ok());
    let mut opts = DiffOptions::new();
    let diff = repo.diff_tree_to_index(
        head_tree.as_ref(),
        None,
        Some(&mut opts),
    );
    if let Ok(diff) = diff {
        collect_diff_stats(&diff, &mut map);
    }
    map
}

fn diff_stats_workdir_to_index(repo: &Repository) -> HashMap<String, (usize, usize)> {
    let mut map = HashMap::new();
    let mut opts = DiffOptions::new();
    let diff = repo.diff_index_to_workdir(None, Some(&mut opts));
    if let Ok(diff) = diff {
        collect_diff_stats(&diff, &mut map);
    }
    map
}

fn collect_diff_stats(diff: &git2::Diff, map: &mut HashMap<String, (usize, usize)>) {
    let num_deltas = diff.deltas().len();
    for i in 0..num_deltas {
        if let Ok(Some(patch)) = git2::Patch::from_diff(diff, i) {
            let path = patch.delta().new_file().path()
                .or_else(|| patch.delta().old_file().path())
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            if path.is_empty() { continue; }
            let (_, ins, del) = patch.line_stats().unwrap_or((0, 0, 0));
            map.insert(path, (ins, del));
        }
    }
}
