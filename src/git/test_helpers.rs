use git2::{Repository, Signature};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// A temporary git repository for testing.
pub struct TestRepo {
    // Hold TempDir so it isn't dropped (and deleted) while tests run.
    #[allow(dead_code)]
    dir: TempDir,
    pub repo: Repository,
}

impl TestRepo {
    /// Create a new empty git repository with user config set.
    pub fn new() -> Self {
        let dir = TempDir::new().expect("create temp dir");
        let repo = Repository::init(dir.path()).expect("git init");
        {
            let mut config = repo.config().expect("repo config");
            config.set_str("user.name", "Test User").unwrap();
            config.set_str("user.email", "test@example.com").unwrap();
        }
        Self { dir, repo }
    }

    /// Create a repo with one initial commit containing `hello.txt`.
    pub fn with_initial_commit() -> Self {
        let tr = Self::new();
        tr.add_and_commit("hello.txt", "hello\n", "Initial commit");
        tr
    }

    /// Write a file relative to the workdir.
    pub fn write_file(&self, path: &str, content: &str) {
        let full = self.workdir().join(path);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    /// Write, stage, and commit a file in one step.
    pub fn add_and_commit(&self, path: &str, content: &str, message: &str) {
        self.write_file(path, content);
        let mut index = self.repo.index().unwrap();
        index.add_path(Path::new(path)).unwrap();
        index.write().unwrap();
        let tree_oid = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_oid).unwrap();
        let sig = Signature::now("Test User", "test@example.com").unwrap();
        let parent = self
            .repo
            .head()
            .ok()
            .and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        self.repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
    }

    pub fn workdir(&self) -> PathBuf {
        self.repo.workdir().expect("not bare").to_path_buf()
    }

    pub fn path_str(&self) -> String {
        self.workdir().to_string_lossy().into_owned()
    }
}
