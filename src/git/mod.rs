mod diff;
mod log;
mod operations;
mod status;

use color_eyre::Result;
use git2::Repository;
use std::path::Path;

pub use diff::{apply_hunk, apply_lines, changed_rows_in_hunk, compute_diff, DiffLine, DiffLineKind, Hunk, LinePair};
pub use log::{BlameLine, LogEntry};
pub use operations::{BranchEntry, StashEntry};
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

    pub fn get_diff_content(&self, path: &str, staged: bool) -> Result<(String, String)> {
        diff::get_diff_content(&self.repo, path, staged)
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

    pub fn get_commit_diff(&self, hash: &str) -> Result<String> {
        log::get_commit_diff(&self.repo, hash)
    }

    pub fn get_blame(&self, path: &str) -> Result<Vec<BlameLine>> {
        log::get_blame(&self.repo, path)
    }

    pub fn workdir(&self) -> &Path {
        self.repo.workdir().expect("bare repositories not supported")
    }

    pub fn stash_save(&mut self, message: Option<&str>) -> Result<()> {
        operations::stash_save(&mut self.repo, message)
    }

    pub fn stash_pop(&mut self, index: usize) -> Result<()> {
        operations::stash_pop(&mut self.repo, index)
    }

    pub fn stash_apply(&mut self, index: usize) -> Result<()> {
        operations::stash_apply(&mut self.repo, index)
    }

    pub fn stash_drop(&mut self, index: usize) -> Result<()> {
        operations::stash_drop(&mut self.repo, index)
    }

    pub fn stash_list(&mut self) -> Result<Vec<StashEntry>> {
        operations::stash_list(&mut self.repo)
    }

    pub fn list_branches(&self) -> Result<Vec<BranchEntry>> {
        operations::list_branches(&self.repo)
    }

    pub fn checkout_branch(&self, name: &str) -> Result<()> {
        operations::checkout_branch(&self.repo, name)
    }

    pub fn create_branch(&self, name: &str) -> Result<()> {
        operations::create_branch(&self.repo, name)
    }

    pub fn push(&self) -> Result<String> {
        operations::git_push(self.workdir())
    }

    pub fn pull(&self) -> Result<String> {
        operations::git_pull(self.workdir())
    }

    pub fn fetch(&self) -> Result<String> {
        operations::git_fetch(self.workdir())
    }

    /// Returns (ahead, behind) relative to the upstream tracking branch.
    /// Returns (0, 0) if no upstream is configured.
    pub fn ahead_behind(&self) -> (usize, usize) {
        let head = match self.repo.head() {
            Ok(h) => h,
            Err(_) => return (0, 0),
        };
        let local_oid = match head.target() {
            Some(oid) => oid,
            None => return (0, 0),
        };
        let branch_name = match head.shorthand() {
            Some(n) => n.to_string(),
            None => return (0, 0),
        };
        let branch = match self.repo.find_branch(&branch_name, git2::BranchType::Local) {
            Ok(b) => b,
            Err(_) => return (0, 0),
        };
        let upstream = match branch.upstream() {
            Ok(u) => u,
            Err(_) => return (0, 0),
        };
        let upstream_oid = match upstream.get().target() {
            Some(oid) => oid,
            None => return (0, 0),
        };
        self.repo.graph_ahead_behind(local_oid, upstream_oid).unwrap_or((0, 0))
    }
}
