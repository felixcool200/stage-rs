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
        .or_else(|_| Signature::now("gitview-rs", "gitview@localhost"))
        .map_err(|e| eyre!("Cannot create signature: {e}"))?;

    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(_) => None, // Initial commit
    };

    let parents: Vec<&git2::Commit> = parent.iter().collect();
    let oid = repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)?;

    Ok(oid.to_string()[..7].to_string())
}

pub fn commit_amend(repo: &Repository, message: &str) -> Result<String> {
    let sig = repo
        .signature()
        .or_else(|_| Signature::now("gitview-rs", "gitview@localhost"))
        .map_err(|e| eyre!("Cannot create signature: {e}"))?;

    let head = repo.head().map_err(|_| eyre!("No HEAD to amend"))?;
    let head_commit = head.peel_to_commit()?;

    let mut index = repo.index()?;
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let oid = head_commit.amend(Some("HEAD"), Some(&sig), Some(&sig), None, Some(message), Some(&tree))?;

    Ok(oid.to_string()[..7].to_string())
}

/// Soft-reset HEAD to HEAD~1, returning the commit message of the undone commit.
pub fn undo_last_commit(repo: &Repository) -> Result<String> {
    let head = repo.head().map_err(|_| eyre!("No HEAD to undo"))?;
    let head_commit = head.peel_to_commit()?;
    let message = head_commit
        .message()
        .unwrap_or("")
        .to_string();

    let parent = head_commit
        .parent(0)
        .map_err(|_| eyre!("Cannot undo initial commit"))?;

    // Soft reset: move HEAD to parent, keep index and working tree
    repo.reset(
        parent.as_object(),
        git2::ResetType::Soft,
        None,
    )?;

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
        .or_else(|_| Signature::now("gitview-rs", "gitview@localhost"))
        .map_err(|e| eyre!("Cannot create signature: {e}"))?;
    let msg = message.unwrap_or("gitview-rs stash");
    repo.stash_save(&sig, msg, Some(git2::StashFlags::DEFAULT))?;
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
        entries.push(BranchEntry { name, is_current, is_remote: false });
    }
    // Also list remote branches
    let remote_branches = repo.branches(Some(git2::BranchType::Remote))?;
    for b in remote_branches {
        let (branch, _) = b?;
        let name = branch.name()?.unwrap_or("").to_string();
        entries.push(BranchEntry { name, is_current: false, is_remote: true });
    }
    Ok(entries)
}

pub fn checkout_branch(repo: &Repository, name: &str) -> Result<()> {
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

pub fn git_push(workdir: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["push"])
        .current_dir(workdir)
        .output()
        .map_err(|e| eyre!("Failed to run git push: {e}"))?;
    if output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Ok(if msg.is_empty() { "Pushed successfully".into() } else { msg })
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(eyre!("{err}"))
    }
}

pub fn git_pull(workdir: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["pull"])
        .current_dir(workdir)
        .output()
        .map_err(|e| eyre!("Failed to run git pull: {e}"))?;
    if output.status.success() {
        let msg = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(if msg.is_empty() { "Pulled successfully".into() } else { msg })
    } else {
        let err = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(eyre!("{err}"))
    }
}

pub fn git_fetch(workdir: &Path) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["fetch", "--all"])
        .current_dir(workdir)
        .output()
        .map_err(|e| eyre!("Failed to run git fetch: {e}"))?;
    if output.status.success() {
        let msg = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Ok(if msg.is_empty() { "Fetched successfully".into() } else { msg })
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
