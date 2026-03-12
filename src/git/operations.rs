use color_eyre::Result;
use git2::{build::CheckoutBuilder, Repository};
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

pub fn unstage_file(repo: &Repository, path: &str) -> Result<()> {
    let mut index = repo.index()?;

    if let Ok(head) = repo.head() {
        let tree = head.peel_to_tree()?;
        if let Ok(entry) = tree.get_path(Path::new(path)) {
            // File exists in HEAD — reset the index entry to match HEAD
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
            // File doesn't exist in HEAD — it was newly staged, remove from index
            index.remove_path(Path::new(path))?;
        }
    } else {
        // No HEAD (empty repo) — remove from index
        index.remove_path(Path::new(path))?;
    }

    index.write()?;
    Ok(())
}

pub fn discard_changes(repo: &Repository, path: &str) -> Result<()> {
    repo.checkout_head(Some(
        CheckoutBuilder::new()
            .path(path)
            .force(),
    ))?;
    Ok(())
}
