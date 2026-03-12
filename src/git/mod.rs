mod diff;
mod log;
mod operations;
mod status;

use color_eyre::Result;
use git2::Repository;
use std::path::Path;

pub use diff::{apply_hunk, apply_lines, changed_rows_in_hunk, compute_diff, DiffLine, DiffLineKind, Hunk, LinePair};
pub use log::LogEntry;
pub use status::{ChangeKind, FileEntry, FileStatus};

pub struct GitRepo {
    pub repo: Repository,
}

impl GitRepo {
    pub fn open(path: &str) -> Result<Self> {
        let repo = Repository::discover(path)?;
        Ok(Self { repo })
    }

    pub fn branch_name(&self) -> String {
        self.repo
            .head()
            .ok()
            .and_then(|h| h.shorthand().map(String::from))
            .unwrap_or_else(|| "(detached)".into())
    }

    pub fn get_file_statuses(&self) -> Result<Vec<FileEntry>> {
        status::get_file_statuses(&self.repo)
    }

    pub fn get_diff_content(&self, path: &str) -> Result<(String, String)> {
        diff::get_diff_content(&self.repo, path)
    }

    pub fn stage_file(&self, path: &str) -> Result<()> {
        operations::stage_file(&self.repo, path)
    }

    pub fn unstage_file(&self, path: &str) -> Result<()> {
        operations::unstage_file(&self.repo, path)
    }

    pub fn discard_changes(&self, path: &str) -> Result<()> {
        operations::discard_changes(&self.repo, path)
    }

    pub fn stage_content(&self, path: &str, content: &str) -> Result<()> {
        operations::stage_content(&self.repo, path, content)
    }

    pub fn commit(&self, message: &str) -> Result<String> {
        operations::commit(&self.repo, message)
    }

    pub fn commit_amend(&self, message: &str) -> Result<String> {
        operations::commit_amend(&self.repo, message)
    }

    pub fn undo_last_commit(&self) -> Result<String> {
        operations::undo_last_commit(&self.repo)
    }

    pub fn last_commit_message(&self) -> Option<String> {
        operations::last_commit_message(&self.repo)
    }

    pub fn has_staged_changes(&self) -> bool {
        operations::has_staged_changes(&self.repo)
    }

    pub fn get_log(&self, max_count: usize) -> Result<Vec<LogEntry>> {
        log::get_log(&self.repo, max_count)
    }

    pub fn workdir(&self) -> &Path {
        self.repo.workdir().expect("bare repositories not supported")
    }
}
