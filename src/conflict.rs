pub struct ConflictState {
    pub file_path: String,
    pub sections: Vec<ConflictSection>,
    pub current_section: usize,
    /// Lines before first conflict
    pub prefix: Vec<String>,
    /// Branch name from <<<<<<< marker (e.g. "HEAD" or "master")
    pub left_name: String,
    /// Branch name from >>>>>>> marker (e.g. "feature-branch")
    pub right_name: String,
}

#[derive(Clone)]
pub struct ConflictSection {
    pub ours: Vec<String>,
    pub theirs: Vec<String>,
    pub resolution: ConflictResolution,
    /// Lines between this conflict and the next
    pub suffix: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConflictResolution {
    Unresolved,
    Ours,
    Theirs,
    Both,
}

pub(crate) struct ParsedConflicts {
    pub prefix: Vec<String>,
    pub sections: Vec<ConflictSection>,
    pub left_name: String,
    pub right_name: String,
}

pub fn parse_conflicts(content: &str) -> Option<ParsedConflicts> {
    let lines: Vec<&str> = content.lines().collect();
    let mut sections = Vec::new();
    let mut prefix = Vec::new();
    let mut i = 0;
    let mut in_prefix = true;
    let mut left_name = String::new();
    let mut right_name = String::new();

    while i < lines.len() {
        if lines[i].starts_with("<<<<<<<") {
            in_prefix = false;
            // Extract left branch name from "<<<<<<< NAME"
            if left_name.is_empty() {
                left_name = lines[i].trim_start_matches('<').trim().to_string();
            }
            let mut ours = Vec::new();
            i += 1;
            while i < lines.len() && !lines[i].starts_with("=======") {
                ours.push(lines[i].to_string());
                i += 1;
            }
            i += 1; // skip =======
            let mut theirs = Vec::new();
            while i < lines.len() && !lines[i].starts_with(">>>>>>>") {
                theirs.push(lines[i].to_string());
                i += 1;
            }
            // Extract right branch name from ">>>>>>> NAME"
            if i < lines.len() && right_name.is_empty() {
                right_name = lines[i].trim_start_matches('>').trim().to_string();
            }
            i += 1; // skip >>>>>>>
                    // Collect suffix lines until next conflict or end
            let mut suffix = Vec::new();
            while i < lines.len() && !lines[i].starts_with("<<<<<<<") {
                suffix.push(lines[i].to_string());
                i += 1;
            }
            sections.push(ConflictSection {
                ours,
                theirs,
                resolution: ConflictResolution::Unresolved,
                suffix,
            });
        } else {
            if in_prefix {
                prefix.push(lines[i].to_string());
            }
            i += 1;
        }
    }

    if sections.is_empty() {
        None
    } else {
        if left_name.is_empty() {
            left_name = "left".to_string();
        }
        if right_name.is_empty() {
            right_name = "right".to_string();
        }
        Some(ParsedConflicts {
            prefix,
            sections,
            left_name,
            right_name,
        })
    }
}
