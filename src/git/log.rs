use color_eyre::Result;
use git2::Repository;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub hash: String,
    pub author: String,
    pub date: String,
    pub message: String,
    pub refs: Vec<String>,
}

pub fn get_log(repo: &Repository, max_count: usize) -> Result<Vec<LogEntry>> {
    // Build a map of commit OID -> ref names (branches and tags)
    let mut ref_map: std::collections::HashMap<git2::Oid, Vec<String>> = std::collections::HashMap::new();
    if let Ok(refs) = repo.references() {
        for reference in refs.flatten() {
            let name = if let Some(shorthand) = reference.shorthand() {
                shorthand.to_string()
            } else {
                continue;
            };
            // Resolve to the commit OID (handles annotated tags too)
            if let Ok(target) = reference.peel_to_commit() {
                ref_map.entry(target.id()).or_default().push(name);
            }
        }
    }

    let mut revwalk = repo.revwalk()?;
    revwalk.push_head()?;
    revwalk.set_sorting(git2::Sort::TIME)?;

    let mut entries = Vec::new();
    for oid in revwalk.take(max_count).flatten() {
        let commit = repo.find_commit(oid)?;
        let time = commit.time();
        let secs = time.seconds();
        let date = format_timestamp(secs);

        entries.push(LogEntry {
            hash: oid.to_string()[..7].to_string(),
            author: commit
                .author()
                .name()
                .unwrap_or("unknown")
                .to_string(),
            date,
            message: commit
                .summary()
                .unwrap_or("")
                .to_string(),
            refs: ref_map.remove(&oid).unwrap_or_default(),
        });
    }

    Ok(entries)
}

/// Get the diff for a specific commit as a string.
pub fn get_commit_diff(repo: &Repository, hash: &str) -> Result<String> {
    let obj = repo.revparse_single(hash)
        .map_err(|e| color_eyre::eyre::eyre!("Cannot find commit: {e}"))?;
    let commit = obj.peel_to_commit()
        .map_err(|e| color_eyre::eyre::eyre!("Not a commit: {e}"))?;
    let tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

    let mut output = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            'H' => "@@",
            'F' => "--- ",
            _ => " ",
        };
        output.push_str(prefix);
        output.push_str(&String::from_utf8_lossy(line.content()));
        true
    })?;
    Ok(output)
}

#[derive(Debug, Clone)]
pub struct BlameLine {
    pub hash: String,
    pub author: String,
}

pub fn get_blame(repo: &Repository, path: &str) -> Result<Vec<BlameLine>> {
    let spec = repo.blame_file(std::path::Path::new(path), None)?;
    let mut lines = Vec::new();
    for i in 0..spec.len() {
        let hunk = spec.get_index(i).unwrap();
        let oid = hunk.final_commit_id();
        let hash = oid.to_string()[..7].to_string();
        let author = repo.find_commit(oid)
            .ok()
            .and_then(|c| c.author().name().map(String::from))
            .unwrap_or_default();
        let count = hunk.lines_in_hunk();
        for _ in 0..count {
            lines.push(BlameLine {
                hash: hash.clone(),
                author: author.clone(),
            });
        }
    }
    Ok(lines)
}

fn format_timestamp(secs: i64) -> String {
    // Simple timestamp formatting without chrono dependency
    // Unix epoch: 1970-01-01
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;

    // Calculate year/month/day from days since epoch
    let mut y = 1970;
    let mut remaining = days;

    loop {
        let days_in_year = if is_leap(y) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }

    let month_days = if is_leap(y) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut m = 0;
    for days_in_month in month_days {
        if remaining < days_in_month {
            break;
        }
        remaining -= days_in_month;
        m += 1;
    }

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        y,
        m + 1,
        remaining + 1,
        hours,
        minutes
    )
}

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}
