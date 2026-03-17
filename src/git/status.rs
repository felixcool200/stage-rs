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
            FileStatus::Staged(_) => 2,
            FileStatus::Untracked => 3,
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
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Added),
                insertions: 0,
                deletions: 0,
            });
        } else if s.contains(Status::INDEX_MODIFIED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Modified),
                insertions: 0,
                deletions: 0,
            });
        } else if s.contains(Status::INDEX_DELETED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Deleted),
                insertions: 0,
                deletions: 0,
            });
        } else if s.contains(Status::INDEX_RENAMED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Renamed),
                insertions: 0,
                deletions: 0,
            });
        }

        // Unstaged changes (workdir vs index)
        if s.contains(Status::WT_MODIFIED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Unstaged(ChangeKind::Modified),
                insertions: 0,
                deletions: 0,
            });
        } else if s.contains(Status::WT_DELETED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Unstaged(ChangeKind::Deleted),
                insertions: 0,
                deletions: 0,
            });
        } else if s.contains(Status::WT_RENAMED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Unstaged(ChangeKind::Renamed),
                insertions: 0,
                deletions: 0,
            });
        } else if s.contains(Status::WT_NEW) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Untracked,
                insertions: 0,
                deletions: 0,
            });
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
    let head_tree = repo.head().ok().and_then(|h| h.peel_to_tree().ok());
    let mut opts = DiffOptions::new();
    let diff = repo.diff_tree_to_index(head_tree.as_ref(), None, Some(&mut opts));
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
            let path = patch
                .delta()
                .new_file()
                .path()
                .or_else(|| patch.delta().old_file().path())
                .and_then(|p| p.to_str())
                .unwrap_or("")
                .to_string();
            if path.is_empty() {
                continue;
            }
            let (_, ins, del) = patch.line_stats().unwrap_or((0, 0, 0));
            map.insert(path, (ins, del));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_helpers::TestRepo;

    // ── Pure function tests ──

    #[test]
    fn test_sort_key_ordering() {
        assert!(
            FileStatus::Conflict.sort_key() < FileStatus::Unstaged(ChangeKind::Modified).sort_key()
        );
        assert!(
            FileStatus::Unstaged(ChangeKind::Modified).sort_key()
                < FileStatus::Staged(ChangeKind::Modified).sort_key()
        );
        assert!(
            FileStatus::Staged(ChangeKind::Modified).sort_key()
                < FileStatus::Untracked.sort_key()
        );
    }

    #[test]
    fn test_short_label_all_variants() {
        assert_eq!(FileStatus::Staged(ChangeKind::Modified).short_label(), "M");
        assert_eq!(FileStatus::Unstaged(ChangeKind::Added).short_label(), "A");
        assert_eq!(FileStatus::Staged(ChangeKind::Deleted).short_label(), "D");
        assert_eq!(FileStatus::Unstaged(ChangeKind::Renamed).short_label(), "R");
        assert_eq!(FileStatus::Untracked.short_label(), "?");
        assert_eq!(FileStatus::Conflict.short_label(), "C");
    }

    #[test]
    fn test_section_name_all_variants() {
        assert_eq!(FileStatus::Conflict.section_name(), "Merge Conflicts");
        assert_eq!(
            FileStatus::Unstaged(ChangeKind::Modified).section_name(),
            "Changes"
        );
        assert_eq!(FileStatus::Untracked.section_name(), "Untracked");
        assert_eq!(
            FileStatus::Staged(ChangeKind::Added).section_name(),
            "Staged Changes"
        );
    }

    // ── Repo-based tests ──

    #[test]
    fn test_statuses_untracked() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("new.txt", "untracked\n");
        let entries = get_file_statuses(&tr.repo).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "new.txt");
        assert_eq!(entries[0].status, FileStatus::Untracked);
    }

    #[test]
    fn test_statuses_staged_new() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("new.txt", "staged\n");
        let mut index = tr.repo.index().unwrap();
        index.add_path(std::path::Path::new("new.txt")).unwrap();
        index.write().unwrap();
        let entries = get_file_statuses(&tr.repo).unwrap();
        let staged: Vec<_> = entries
            .iter()
            .filter(|e| matches!(e.status, FileStatus::Staged(ChangeKind::Added)))
            .collect();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].path, "new.txt");
    }

    #[test]
    fn test_statuses_staged_modified() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "modified\n");
        let mut index = tr.repo.index().unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();
        let entries = get_file_statuses(&tr.repo).unwrap();
        let staged: Vec<_> = entries
            .iter()
            .filter(|e| matches!(e.status, FileStatus::Staged(ChangeKind::Modified)))
            .collect();
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].path, "hello.txt");
    }

    #[test]
    fn test_statuses_unstaged_modified() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "changed\n");
        let entries = get_file_statuses(&tr.repo).unwrap();
        let unstaged: Vec<_> = entries
            .iter()
            .filter(|e| matches!(e.status, FileStatus::Unstaged(ChangeKind::Modified)))
            .collect();
        assert_eq!(unstaged.len(), 1);
        assert_eq!(unstaged[0].path, "hello.txt");
    }

    #[test]
    fn test_statuses_both_staged_and_unstaged() {
        let tr = TestRepo::with_initial_commit();
        // Stage a modification
        tr.write_file("hello.txt", "staged version\n");
        let mut index = tr.repo.index().unwrap();
        index.add_path(std::path::Path::new("hello.txt")).unwrap();
        index.write().unwrap();
        // Then modify the workdir again
        tr.write_file("hello.txt", "unstaged version\n");
        let entries = get_file_statuses(&tr.repo).unwrap();
        let hello_entries: Vec<_> = entries.iter().filter(|e| e.path == "hello.txt").collect();
        assert_eq!(hello_entries.len(), 2);
        assert!(hello_entries
            .iter()
            .any(|e| matches!(e.status, FileStatus::Staged(ChangeKind::Modified))));
        assert!(hello_entries
            .iter()
            .any(|e| matches!(e.status, FileStatus::Unstaged(ChangeKind::Modified))));
    }

    #[test]
    fn test_statuses_deleted() {
        let tr = TestRepo::with_initial_commit();
        std::fs::remove_file(tr.workdir().join("hello.txt")).unwrap();
        let entries = get_file_statuses(&tr.repo).unwrap();
        let deleted: Vec<_> = entries
            .iter()
            .filter(|e| matches!(e.status, FileStatus::Unstaged(ChangeKind::Deleted)))
            .collect();
        assert_eq!(deleted.len(), 1);
        assert_eq!(deleted[0].path, "hello.txt");
    }

    #[test]
    fn test_statuses_sorted() {
        let tr = TestRepo::with_initial_commit();
        // Create untracked file
        tr.write_file("zzz.txt", "untracked\n");
        // Create unstaged modification
        tr.write_file("hello.txt", "changed\n");
        // Create staged new file
        tr.write_file("aaa.txt", "staged\n");
        let mut index = tr.repo.index().unwrap();
        index.add_path(std::path::Path::new("aaa.txt")).unwrap();
        index.write().unwrap();
        let entries = get_file_statuses(&tr.repo).unwrap();
        // Order: unstaged (hello.txt), untracked (zzz.txt), staged (aaa.txt)
        assert!(entries[0].status.sort_key() <= entries[1].status.sort_key());
        assert!(entries[1].status.sort_key() <= entries[2].status.sort_key());
    }

    #[test]
    fn test_statuses_diff_stats() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "hello\nnew line\n");
        let entries = get_file_statuses(&tr.repo).unwrap();
        let entry = entries.iter().find(|e| e.path == "hello.txt").unwrap();
        assert_eq!(entry.insertions, 1);
        assert_eq!(entry.deletions, 0);
    }
}
