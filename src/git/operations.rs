use color_eyre::{eyre::eyre, Result};
use git2::{build::CheckoutBuilder, Repository, Signature};
use std::path::Path;

pub fn stage_file(repo: &Repository, path: &str) -> Result<()> {
    let mut index = repo.index()?;
    let full_path = repo.workdir().unwrap().join(path);

    if full_path.exists() {
        index.add_path(Path::new(path))?;
    } else {
        index.remove_path(Path::new(path))?;
    }
    index.write()?;
    Ok(())
}

pub fn stage_content(repo: &Repository, path: &str, content: &str) -> Result<()> {
    let blob_oid = repo.blob(content.as_bytes())?;
    let mut index = repo.index()?;

    let mode = index
        .get_path(Path::new(path), 0)
        .map(|e| e.mode)
        .unwrap_or(0o100644);

    let entry = git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: content.len() as u32,
        id: blob_oid,
        flags: 0,
        flags_extended: 0,
        path: path.as_bytes().to_vec(),
    };
    index.add(&entry)?;
    index.write()?;
    Ok(())
}

pub fn unstage_file(repo: &Repository, path: &str) -> Result<()> {
    let mut index = repo.index()?;

    if let Ok(head) = repo.head() {
        let tree = head.peel_to_tree()?;
        if let Ok(entry) = tree.get_path(Path::new(path)) {
            let blob = repo.find_blob(entry.id())?;
            let index_entry = git2::IndexEntry {
                ctime: git2::IndexTime::new(0, 0),
                mtime: git2::IndexTime::new(0, 0),
                dev: 0,
                ino: 0,
                mode: entry.filemode() as u32,
                uid: 0,
                gid: 0,
                file_size: blob.content().len() as u32,
                id: entry.id(),
                flags: 0,
                flags_extended: 0,
                path: path.as_bytes().to_vec(),
            };
            index.add(&index_entry)?;
        } else {
            index.remove_path(Path::new(path))?;
        }
    } else {
        index.remove_path(Path::new(path))?;
    }

    index.write()?;
    Ok(())
}

pub fn discard_changes(repo: &Repository, path: &str) -> Result<()> {
    repo.checkout_head(Some(CheckoutBuilder::new().path(path).force()))?;
    Ok(())
}

pub fn commit(repo: &Repository, message: &str) -> Result<String> {
    let sig = repo
        .signature()
        .or_else(|_| Signature::now("stage-rs", "stage-rs@localhost"))
        .map_err(|e| eyre!("Cannot create signature: {e}"))?;

    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(_) => None, // Initial commit
    };

    let mut parents: Vec<git2::Commit> = parent.into_iter().collect();

    // During a merge, include MERGE_HEAD as a second parent
    if let Ok(merge_head_bytes) = std::fs::read(repo.path().join("MERGE_HEAD")) {
        let merge_head_str = String::from_utf8_lossy(&merge_head_bytes);
        if let Ok(oid) = git2::Oid::from_str(merge_head_str.trim()) {
            if let Ok(commit) = repo.find_commit(oid) {
                parents.push(commit);
            }
        }
    }

    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parent_refs)?;

    // Clean up merge state files after successful merge commit
    let _ = std::fs::remove_file(repo.path().join("MERGE_HEAD"));
    let _ = std::fs::remove_file(repo.path().join("MERGE_MSG"));
    let _ = std::fs::remove_file(repo.path().join("MERGE_MODE"));

    Ok(oid.to_string()[..7].to_string())
}

pub fn commit_amend(repo: &Repository, message: &str) -> Result<String> {
    let sig = repo
        .signature()
        .or_else(|_| Signature::now("stage-rs", "stage-rs@localhost"))
        .map_err(|e| eyre!("Cannot create signature: {e}"))?;

    let head = repo.head().map_err(|_| eyre!("No HEAD to amend"))?;
    let head_commit = head.peel_to_commit()?;

    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let oid = head_commit.amend(
        Some("HEAD"),
        Some(&sig),
        Some(&sig),
        None,
        Some(message),
        Some(&tree),
    )?;

    Ok(oid.to_string()[..7].to_string())
}

/// Soft-reset HEAD to HEAD~1, returning the commit message of the undone commit.
pub fn undo_last_commit(repo: &Repository) -> Result<String> {
    let head = repo.head().map_err(|_| eyre!("No HEAD to undo"))?;
    let head_commit = head.peel_to_commit()?;
    let message = head_commit.message().unwrap_or("").to_string();

    let parent = head_commit
        .parent(0)
        .map_err(|_| eyre!("Cannot undo initial commit"))?;

    // Soft reset: move HEAD to parent, keep index and working tree
    repo.reset(parent.as_object(), git2::ResetType::Soft, None)?;

    Ok(message)
}

pub fn last_commit_message(repo: &Repository) -> Option<String> {
    repo.head()
        .ok()?
        .peel_to_commit()
        .ok()?
        .message()
        .map(|s| s.to_string())
}

// ── Stash ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct StashEntry {
    pub index: usize,
    pub message: String,
}

pub fn stash_save(repo: &mut Repository, message: Option<&str>) -> Result<()> {
    let sig = repo
        .signature()
        .or_else(|_| Signature::now("stage-rs", "stage-rs@localhost"))
        .map_err(|e| eyre!("Cannot create signature: {e}"))?;
    let msg = message.unwrap_or("stage-rs stash");
    repo.stash_save(&sig, msg, Some(git2::StashFlags::INCLUDE_UNTRACKED))?;
    Ok(())
}

pub fn stash_pop(repo: &mut Repository, index: usize) -> Result<()> {
    repo.stash_pop(index, None)?;
    Ok(())
}

pub fn stash_apply(repo: &mut Repository, index: usize) -> Result<()> {
    repo.stash_apply(index, None)?;
    Ok(())
}

pub fn stash_drop(repo: &mut Repository, index: usize) -> Result<()> {
    repo.stash_drop(index)?;
    Ok(())
}

pub fn stash_list(repo: &mut Repository) -> Result<Vec<StashEntry>> {
    let mut entries = Vec::new();
    repo.stash_foreach(|index, message, _oid| {
        entries.push(StashEntry {
            index,
            message: message.to_string(),
        });
        true
    })?;
    Ok(entries)
}

// ── Branches ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct BranchEntry {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
}

pub fn list_branches(repo: &Repository) -> Result<Vec<BranchEntry>> {
    let mut entries = Vec::new();
    let branches = repo.branches(Some(git2::BranchType::Local))?;
    for b in branches {
        let (branch, _) = b?;
        let name = branch.name()?.unwrap_or("").to_string();
        let is_current = branch.is_head();
        entries.push(BranchEntry {
            name,
            is_current,
            is_remote: false,
        });
    }
    // Also list remote branches
    let remote_branches = repo.branches(Some(git2::BranchType::Remote))?;
    for b in remote_branches {
        let (branch, _) = b?;
        let name = branch.name()?.unwrap_or("").to_string();
        entries.push(BranchEntry {
            name,
            is_current: false,
            is_remote: true,
        });
    }
    Ok(entries)
}

pub fn checkout_branch(repo: &Repository, name: &str) -> Result<()> {
    let (obj, reference) = repo.revparse_ext(name)?;
    repo.checkout_tree(&obj, Some(CheckoutBuilder::new().safe()))?;
    match reference {
        Some(r) => repo.set_head(r.name().unwrap_or(&format!("refs/heads/{name}")))?,
        None => repo.set_head_detached(obj.id())?,
    }
    Ok(())
}

pub fn force_checkout_branch(repo: &Repository, name: &str) -> Result<()> {
    let (obj, reference) = repo.revparse_ext(name)?;
    repo.checkout_tree(&obj, Some(CheckoutBuilder::new().force()))?;
    match reference {
        Some(r) => repo.set_head(r.name().unwrap_or(&format!("refs/heads/{name}")))?,
        None => repo.set_head_detached(obj.id())?,
    }
    Ok(())
}

pub fn create_branch(repo: &Repository, name: &str) -> Result<()> {
    let head = repo.head().map_err(|_| eyre!("No HEAD"))?;
    let commit = head.peel_to_commit()?;
    repo.branch(name, &commit, false)?;
    // Checkout the new branch
    checkout_branch(repo, name)?;
    Ok(())
}

// ── Remote Operations ────────────────────────────────────────────────────────

pub fn git_rebase_continue(workdir: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rebase", "--continue"])
        .env("GIT_EDITOR", "true")
        .current_dir(workdir)
        .output()
        .map_err(|e| eyre!("Failed to run git rebase --continue: {e}"))?;
    if output.status.success() {
        Ok("Rebase continue successful".into())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(eyre!("{err}"))
    }
}

pub fn git_rebase_abort(workdir: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rebase", "--abort"])
        .current_dir(workdir)
        .output()
        .map_err(|e| eyre!("Failed to run git rebase --abort: {e}"))?;
    if output.status.success() {
        Ok("Rebase aborted".into())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(eyre!("{err}"))
    }
}

pub fn has_staged_changes(repo: &Repository) -> bool {
    let Ok(statuses) = repo.statuses(None) else {
        return false;
    };
    statuses.iter().any(|e| {
        let s = e.status();
        s.intersects(
            git2::Status::INDEX_NEW
                | git2::Status::INDEX_MODIFIED
                | git2::Status::INDEX_DELETED
                | git2::Status::INDEX_RENAMED,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_helpers::TestRepo;

    #[test]
    fn test_stage_file() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("new.txt", "content\n");
        stage_file(&tr.repo, "new.txt").unwrap();
        let index = tr.repo.index().unwrap();
        assert!(index.get_path(Path::new("new.txt"), 0).is_some());
    }

    #[test]
    fn test_stage_file_deleted() {
        let tr = TestRepo::with_initial_commit();
        std::fs::remove_file(tr.workdir().join("hello.txt")).unwrap();
        stage_file(&tr.repo, "hello.txt").unwrap();
        let index = tr.repo.index().unwrap();
        assert!(index.get_path(Path::new("hello.txt"), 0).is_none());
    }

    #[test]
    fn test_stage_content() {
        let tr = TestRepo::with_initial_commit();
        stage_content(&tr.repo, "hello.txt", "patched\n").unwrap();
        let index = tr.repo.index().unwrap();
        let entry = index.get_path(Path::new("hello.txt"), 0).unwrap();
        let blob = tr.repo.find_blob(entry.id).unwrap();
        assert_eq!(std::str::from_utf8(blob.content()).unwrap(), "patched\n");
    }

    #[test]
    fn test_unstage_file_restores_head() {
        let tr = TestRepo::with_initial_commit();
        // Stage a modification
        tr.write_file("hello.txt", "changed\n");
        stage_file(&tr.repo, "hello.txt").unwrap();
        // Unstage it
        unstage_file(&tr.repo, "hello.txt").unwrap();
        let index = tr.repo.index().unwrap();
        let entry = index.get_path(Path::new("hello.txt"), 0).unwrap();
        let blob = tr.repo.find_blob(entry.id).unwrap();
        assert_eq!(std::str::from_utf8(blob.content()).unwrap(), "hello\n");
    }

    #[test]
    fn test_unstage_file_new_file() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("new.txt", "content\n");
        stage_file(&tr.repo, "new.txt").unwrap();
        unstage_file(&tr.repo, "new.txt").unwrap();
        let index = tr.repo.index().unwrap();
        assert!(index.get_path(Path::new("new.txt"), 0).is_none());
    }

    #[test]
    fn test_discard_changes() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "modified\n");
        discard_changes(&tr.repo, "hello.txt").unwrap();
        let content = std::fs::read_to_string(tr.workdir().join("hello.txt")).unwrap();
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn test_commit_creates_commit() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "v2\n");
        stage_file(&tr.repo, "hello.txt").unwrap();
        commit(&tr.repo, "Second commit").unwrap();
        let head = tr.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "Second commit");
        let tree = head.tree().unwrap();
        let entry = tree.get_path(Path::new("hello.txt")).unwrap();
        let blob = tr.repo.find_blob(entry.id()).unwrap();
        assert_eq!(std::str::from_utf8(blob.content()).unwrap(), "v2\n");
    }

    #[test]
    fn test_commit_returns_short_hash() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "v2\n");
        stage_file(&tr.repo, "hello.txt").unwrap();
        let hash = commit(&tr.repo, "test").unwrap();
        assert_eq!(hash.len(), 7);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_commit_initial() {
        let tr = TestRepo::new();
        tr.write_file("first.txt", "first\n");
        stage_file(&tr.repo, "first.txt").unwrap();
        let hash = commit(&tr.repo, "Initial").unwrap();
        assert_eq!(hash.len(), 7);
        let head = tr.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 0);
    }

    #[test]
    fn test_commit_with_merge_head() {
        let tr = TestRepo::with_initial_commit();
        // Detect the default branch name
        let default_branch = tr.repo.head().unwrap().shorthand().unwrap().to_string();
        // Create a second commit on default branch
        tr.add_and_commit("hello.txt", "main\n", "main change");

        // Create branch from initial commit
        let initial = tr
            .repo
            .head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .parent(0)
            .unwrap();
        tr.repo.branch("feature", &initial, false).unwrap();
        checkout_branch(&tr.repo, "feature").unwrap();
        tr.add_and_commit("other.txt", "feature\n", "feature change");
        let feature_head = tr.repo.head().unwrap().peel_to_commit().unwrap().id();

        // Go back to default branch and fake a merge state
        checkout_branch(&tr.repo, &default_branch).unwrap();
        let merge_head_path = tr.repo.path().join("MERGE_HEAD");
        std::fs::write(&merge_head_path, format!("{}\n", feature_head)).unwrap();

        // Stage something and commit
        tr.write_file("hello.txt", "merged\n");
        stage_file(&tr.repo, "hello.txt").unwrap();
        commit(&tr.repo, "Merge commit").unwrap();

        let head = tr.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.parent_count(), 2);
        // MERGE_HEAD should be cleaned up
        assert!(!merge_head_path.exists());
    }

    #[test]
    fn test_commit_amend() {
        let tr = TestRepo::with_initial_commit();
        let old_oid = tr.repo.head().unwrap().peel_to_commit().unwrap().id();
        tr.write_file("hello.txt", "amended\n");
        stage_file(&tr.repo, "hello.txt").unwrap();
        commit_amend(&tr.repo, "Amended message").unwrap();
        let head = tr.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "Amended message");
        assert_ne!(head.id(), old_oid);
    }

    #[test]
    fn test_undo_last_commit() {
        let tr = TestRepo::with_initial_commit();
        tr.add_and_commit("hello.txt", "v2\n", "Second commit");
        let msg = undo_last_commit(&tr.repo).unwrap();
        assert_eq!(msg, "Second commit");
        let head = tr.repo.head().unwrap().peel_to_commit().unwrap();
        assert_eq!(head.message().unwrap(), "Initial commit");
    }

    #[test]
    fn test_undo_initial_commit_fails() {
        let tr = TestRepo::with_initial_commit();
        let result = undo_last_commit(&tr.repo);
        assert!(result.is_err());
    }

    #[test]
    fn test_last_commit_message() {
        let tr = TestRepo::with_initial_commit();
        assert_eq!(last_commit_message(&tr.repo).unwrap(), "Initial commit");
    }

    #[test]
    fn test_has_staged_changes_true() {
        let tr = TestRepo::with_initial_commit();
        tr.write_file("new.txt", "new\n");
        stage_file(&tr.repo, "new.txt").unwrap();
        assert!(has_staged_changes(&tr.repo));
    }

    #[test]
    fn test_has_staged_changes_false() {
        let tr = TestRepo::with_initial_commit();
        assert!(!has_staged_changes(&tr.repo));
    }

    #[test]
    fn test_stash_save_and_list() {
        let mut tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "dirty\n");
        stash_save(&mut tr.repo, None).unwrap();
        // Workdir should be clean
        let content = std::fs::read_to_string(tr.workdir().join("hello.txt")).unwrap();
        assert_eq!(content, "hello\n");
        let entries = stash_list(&mut tr.repo).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_stash_pop() {
        let mut tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "dirty\n");
        stash_save(&mut tr.repo, None).unwrap();
        // Pop succeeds without error
        stash_pop(&mut tr.repo, 0).unwrap();
        // Stash list should be empty after pop
        let entries = stash_list(&mut tr.repo).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_stash_apply() {
        let mut tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "dirty\n");
        stash_save(&mut tr.repo, None).unwrap();
        // Apply succeeds without error
        stash_apply(&mut tr.repo, 0).unwrap();
        // Stash still exists after apply (unlike pop)
        let entries = stash_list(&mut tr.repo).unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_stash_drop() {
        let mut tr = TestRepo::with_initial_commit();
        tr.write_file("hello.txt", "dirty\n");
        stash_save(&mut tr.repo, None).unwrap();
        stash_drop(&mut tr.repo, 0).unwrap();
        let entries = stash_list(&mut tr.repo).unwrap();
        assert!(entries.is_empty());
        // Workdir still clean (stash was not popped)
        let content = std::fs::read_to_string(tr.workdir().join("hello.txt")).unwrap();
        assert_eq!(content, "hello\n");
    }

    #[test]
    fn test_list_branches() {
        let tr = TestRepo::with_initial_commit();
        let head_commit = tr.repo.head().unwrap().peel_to_commit().unwrap();
        tr.repo.branch("feature", &head_commit, false).unwrap();
        let branches = list_branches(&tr.repo).unwrap();
        let local: Vec<_> = branches.iter().filter(|b| !b.is_remote).collect();
        assert!(local.len() >= 2);
        assert!(local.iter().any(|b| b.name == "main" || b.name == "master"));
        assert!(local.iter().any(|b| b.name == "feature"));
        let current = local.iter().find(|b| b.is_current).unwrap();
        assert!(current.name == "main" || current.name == "master");
    }

    #[test]
    fn test_checkout_branch() {
        let tr = TestRepo::with_initial_commit();
        let head_commit = tr.repo.head().unwrap().peel_to_commit().unwrap();
        tr.repo.branch("feature", &head_commit, false).unwrap();
        checkout_branch(&tr.repo, "feature").unwrap();
        let name = tr.repo.head().unwrap().shorthand().unwrap().to_string();
        assert_eq!(name, "feature");
    }

    #[test]
    fn test_create_branch() {
        let tr = TestRepo::with_initial_commit();
        create_branch(&tr.repo, "new-branch").unwrap();
        let name = tr.repo.head().unwrap().shorthand().unwrap().to_string();
        assert_eq!(name, "new-branch");
    }

    #[test]
    fn test_force_checkout_branch() {
        let tr = TestRepo::with_initial_commit();
        let head_commit = tr.repo.head().unwrap().peel_to_commit().unwrap();
        tr.repo.branch("feature", &head_commit, false).unwrap();
        // Dirty the workdir
        tr.write_file("hello.txt", "dirty\n");
        force_checkout_branch(&tr.repo, "feature").unwrap();
        let name = tr.repo.head().unwrap().shorthand().unwrap().to_string();
        assert_eq!(name, "feature");
    }
}
