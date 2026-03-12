use color_eyre::Result;
use git2::{Repository, Status, StatusOptions};

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: String,
    pub status: FileStatus,
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
            });
            continue;
        }

        // Staged changes (index vs HEAD)
        if s.contains(Status::INDEX_NEW) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Added),
            });
        } else if s.contains(Status::INDEX_MODIFIED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Modified),
            });
        } else if s.contains(Status::INDEX_DELETED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Deleted),
            });
        } else if s.contains(Status::INDEX_RENAMED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Staged(ChangeKind::Renamed),
            });
        }

        // Unstaged changes (workdir vs index)
        if s.contains(Status::WT_MODIFIED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Unstaged(ChangeKind::Modified),
            });
        } else if s.contains(Status::WT_DELETED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Unstaged(ChangeKind::Deleted),
            });
        } else if s.contains(Status::WT_RENAMED) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Unstaged(ChangeKind::Renamed),
            });
        } else if s.contains(Status::WT_NEW) {
            entries.push(FileEntry {
                path: path.clone(),
                status: FileStatus::Untracked,
            });
        }
    }

    entries.sort_by(|a, b| {
        a.status
            .sort_key()
            .cmp(&b.status.sort_key())
            .then(a.path.cmp(&b.path))
    });

    Ok(entries)
}
