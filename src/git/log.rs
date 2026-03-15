use super::diff::{compute_diff, DiffLine, DiffLineKind, Hunk};
use crate::syntax;
use color_eyre::Result;
use git2::Repository;
use std::path::Path;

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

/// Result of computing a commit's side-by-side diff.
pub struct CommitDiffResult {
    pub left_lines: Vec<DiffLine>,
    pub right_lines: Vec<DiffLine>,
    pub hunks: Vec<Hunk>,
    /// File extension for each display line (for syntax highlighting).
    pub file_extensions: Vec<Option<String>>,
}

/// Get the diff for a specific commit as side-by-side DiffLine vectors.
pub fn get_commit_diff_sides(
    repo: &Repository,
    hash: &str,
) -> Result<CommitDiffResult> {
    let obj = repo
        .revparse_single(hash)
        .map_err(|e| color_eyre::eyre::eyre!("Cannot find commit: {e}"))?;
    let commit = obj
        .peel_to_commit()
        .map_err(|e| color_eyre::eyre::eyre!("Not a commit: {e}"))?;
    let tree = commit.tree()?;
    let parent_tree = commit.parent(0).ok().and_then(|p| p.tree().ok());
    let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;

    let mut left_lines = Vec::new();
    let mut right_lines = Vec::new();
    let mut all_hunks = Vec::new();
    let mut file_extensions = Vec::new();
    let num_deltas = diff.deltas().len();

    for (di, delta) in diff.deltas().enumerate() {
        let path = delta
            .new_file()
            .path()
            .or_else(|| delta.old_file().path())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());

        let ext = syntax::file_extension(&path).map(String::from);

        // File header line
        let header = DiffLine {
            content: format!("── {} ──", path),
            kind: DiffLineKind::Equal,
            hunk_index: None,
        };
        left_lines.push(header.clone());
        right_lines.push(header);
        file_extensions.push(None); // no highlighting for header

        // Get old content from parent tree
        let old_content = parent_tree
            .as_ref()
            .and_then(|pt| pt.get_path(Path::new(&path)).ok())
            .and_then(|entry| repo.find_blob(entry.id()).ok())
            .map(|blob| String::from_utf8_lossy(blob.content()).to_string())
            .unwrap_or_default();

        // Get new content from commit tree
        let new_content = tree
            .get_path(Path::new(&path))
            .ok()
            .and_then(|entry| repo.find_blob(entry.id()).ok())
            .map(|blob| String::from_utf8_lossy(blob.content()).to_string())
            .unwrap_or_default();

        let file_offset = left_lines.len();
        let (file_left, file_right, file_hunks) = compute_diff(&old_content, &new_content);

        // Offset hunks to global line indices
        for h in file_hunks {
            all_hunks.push(Hunk {
                display_start: h.display_start + file_offset,
                display_end: h.display_end + file_offset,
            });
        }

        // File extension for each line in this file
        for _ in 0..file_left.len() {
            file_extensions.push(ext.clone());
        }

        left_lines.extend(file_left);
        right_lines.extend(file_right);

        // Blank separator between files
        if di + 1 < num_deltas {
            let sep = DiffLine {
                content: String::new(),
                kind: DiffLineKind::Equal,
                hunk_index: None,
            };
            left_lines.push(sep.clone());
            right_lines.push(sep);
            file_extensions.push(None);
        }
    }

    Ok(CommitDiffResult {
        left_lines,
        right_lines,
        hunks: all_hunks,
        file_extensions,
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_helpers::TestRepo;

    // ── Pure function tests (no repo) ──

    #[test]
    fn test_is_leap_common_years() {
        assert!(!is_leap(1900));
        assert!(!is_leap(2001));
        assert!(!is_leap(2100));
    }

    #[test]
    fn test_is_leap_leap_years() {
        assert!(is_leap(2000));
        assert!(is_leap(2004));
        assert!(is_leap(2024));
    }

    #[test]
    fn test_format_timestamp_epoch() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00");
    }

    #[test]
    fn test_format_timestamp_known_date() {
        // 2024-01-01 00:00 UTC
        assert_eq!(format_timestamp(1704067200), "2024-01-01 00:00");
    }

    #[test]
    fn test_format_timestamp_end_of_year() {
        // 2024-12-31 23:59 UTC
        assert_eq!(format_timestamp(1735689540), "2024-12-31 23:59");
    }

    #[test]
    fn test_format_timestamp_leap_day() {
        // 2024-02-29 12:00 UTC
        assert_eq!(format_timestamp(1709208000), "2024-02-29 12:00");
    }

    #[test]
    fn test_format_timestamp_march_1_non_leap() {
        // 2023-03-01 00:00 UTC
        assert_eq!(format_timestamp(1677628800), "2023-03-01 00:00");
    }

    #[test]
    fn test_format_timestamp_negative() {
        // Pre-1970: documents behavior (may produce unusual output)
        let result = format_timestamp(-86400);
        // Just ensure it doesn't panic; the exact output for negative is undefined
        assert!(!result.is_empty());
    }

    // ── Repo-based tests ──

    #[test]
    fn test_get_log_returns_entries() {
        let tr = TestRepo::with_initial_commit();
        tr.add_and_commit("hello.txt", "v2\n", "Second commit");
        tr.add_and_commit("hello.txt", "v3\n", "Third commit");
        let entries = get_log(&tr.repo, 100).unwrap();
        assert_eq!(entries.len(), 3);
        // All messages should be present
        let messages: Vec<&str> = entries.iter().map(|e| e.message.as_str()).collect();
        assert!(messages.contains(&"Initial commit"));
        assert!(messages.contains(&"Second commit"));
        assert!(messages.contains(&"Third commit"));
    }

    #[test]
    fn test_get_log_max_count() {
        let tr = TestRepo::with_initial_commit();
        tr.add_and_commit("hello.txt", "v2\n", "Second");
        tr.add_and_commit("hello.txt", "v3\n", "Third");
        let entries = get_log(&tr.repo, 2).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_get_log_includes_refs() {
        let tr = TestRepo::with_initial_commit();
        let entries = get_log(&tr.repo, 10).unwrap();
        // The HEAD commit should have the branch name in refs
        let head_entry = &entries[0];
        let has_branch_ref = head_entry.refs.iter().any(|r| r == "main" || r == "master");
        assert!(has_branch_ref, "refs: {:?}", head_entry.refs);
    }

    #[test]
    fn test_get_commit_diff_sides() {
        let tr = TestRepo::with_initial_commit();
        tr.add_and_commit("hello.txt", "hello\nworld\n", "Add world");
        let entries = get_log(&tr.repo, 1).unwrap();
        let result = get_commit_diff_sides(&tr.repo, &entries[0].hash).unwrap();
        assert!(!result.left_lines.is_empty());
        assert!(!result.right_lines.is_empty());
        assert_eq!(result.left_lines.len(), result.right_lines.len());
    }

    #[test]
    fn test_get_blame() {
        let tr = TestRepo::with_initial_commit();
        let lines = get_blame(&tr.repo, "hello.txt").unwrap();
        assert_eq!(lines.len(), 1); // "hello\n" is one line
        assert_eq!(lines[0].hash.len(), 7);
        assert_eq!(lines[0].author, "Test User");
    }
}
