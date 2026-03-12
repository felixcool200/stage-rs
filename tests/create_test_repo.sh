#!/usr/bin/env bash
# Creates a throwaway git repo with various file states for testing stage-rs.
# Usage:
#   ./tests/create_test_repo.sh [target_dir]
#
# Default target: /tmp/stage-rs-test-repo
# Run stage-rs against it:
#   cargo run -- /tmp/stage-rs-test-repo

set -euo pipefail

TARGET="${1:-/tmp/stage-rs-test-repo}"

rm -rf "$TARGET"
mkdir -p "$TARGET"
cd "$TARGET"

git init
git config user.email "test@example.com"
git config user.name "Test User"

# ── Initial commit with several file types ──────────────────────────────────

cat > main.rs <<'EOF'
fn main() {
    println!("Hello, world!");
    let x = 42;
    let y = x + 1;
    println!("x = {x}, y = {y}");
}
EOF

cat > lib.rs <<'EOF'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5, 3), 2);
    }
}
EOF

cat > config.toml <<'EOF'
[package]
name = "demo"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }
EOF

cat > README.md <<'EOF'
# Demo Project

A small demo project for testing stage-rs.

## Features
- Basic math operations
- Configuration file
- Multiple file types
EOF

cat > style.css <<'EOF'
body {
    font-family: sans-serif;
    margin: 0;
    padding: 20px;
    background: #1a1a2e;
    color: #eee;
}

.container {
    max-width: 800px;
    margin: 0 auto;
}

h1 {
    color: #e94560;
}
EOF

cat > data.json <<'EOF'
{
    "name": "test-project",
    "version": "1.0.0",
    "entries": [
        { "id": 1, "label": "alpha" },
        { "id": 2, "label": "beta" },
        { "id": 3, "label": "gamma" }
    ]
}
EOF

mkdir -p src
cat > src/utils.py <<'EOF'
def greet(name: str) -> str:
    return f"Hello, {name}!"

def fibonacci(n: int) -> list[int]:
    if n <= 0:
        return []
    if n == 1:
        return [0]
    seq = [0, 1]
    for _ in range(2, n):
        seq.append(seq[-1] + seq[-2])
    return seq

class Calculator:
    def __init__(self):
        self.history = []

    def add(self, a, b):
        result = a + b
        self.history.append(f"{a} + {b} = {result}")
        return result
EOF

git add -A
git commit -m "Initial commit: multi-language demo project"

# ── Second commit: add more content ─────────────────────────────────────────

cat > Makefile <<'EOF'
.PHONY: build test clean

build:
	cargo build --release

test:
	cargo test

clean:
	cargo clean
EOF

cat >> lib.rs <<'EOF'

pub fn divide(a: i32, b: i32) -> Option<i32> {
    if b == 0 {
        None
    } else {
        Some(a / b)
    }
}
EOF

git add -A
git commit -m "Add divide function and Makefile"

# ── Create a branch for merge conflict ──────────────────────────────────────

git checkout -b feature-refactor

# Modify files on the feature branch
cat > main.rs <<'EOF'
use std::io;

fn main() {
    println!("Welcome to the calculator!");
    let x = 42;
    let y = x * 2;
    println!("x = {x}, y = {y}");

    println!("Enter a number:");
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let n: i32 = input.trim().parse().unwrap_or(0);
    println!("You entered: {n}");
}
EOF

cat > config.toml <<'EOF'
[package]
name = "calculator"
version = "0.2.0"
edition = "2021"

[dependencies]
serde = { version = "1.0", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
anyhow = "1.0"
EOF

git add -A
git commit -m "Refactor: calculator with user input"

# ── Go back to main, make conflicting changes ──────────────────────────────

git checkout master

cat > main.rs <<'EOF'
fn main() {
    println!("Hello from the demo app!");
    let x = 42;
    let z = x.pow(2);
    println!("x = {x}, x^2 = {z}");
    for i in 0..5 {
        println!("  iteration {i}");
    }
}
EOF

cat > config.toml <<'EOF'
[package]
name = "demo-app"
version = "0.1.1"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
EOF

git add -A
git commit -m "Update demo app with loop and clap"

# ── Create a branch for rebase conflict testing ─────────────────────────────
# Branch off from the commit before master's last (so rebasing onto master
# will conflict on lib.rs and data.json — two separate commits, two conflicts)

git checkout -b rebase-test HEAD~1

# First conflicting commit on rebase-test: change lib.rs differently
cat > lib.rs <<'EOF'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

pub fn power(base: i32, exp: u32) -> i32 {
    base.pow(exp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5, 3), 2);
    }

    #[test]
    fn test_power() {
        assert_eq!(power(2, 3), 8);
    }
}
EOF
git add lib.rs
git commit -m "Add power function to lib"

# Second conflicting commit: change data.json differently
cat > data.json <<'EOF'
{
    "name": "test-project",
    "version": "3.0.0-beta",
    "entries": [
        { "id": 1, "label": "one" },
        { "id": 2, "label": "two" },
        { "id": 3, "label": "three" },
        { "id": 4, "label": "four" },
        { "id": 5, "label": "five" }
    ]
}
EOF
git add data.json
git commit -m "Rename entries and bump to v3 beta"

# Go back to master
git checkout master

# ── Merge to create conflicts ───────────────────────────────────────────────

git merge feature-refactor --no-commit --no-ff 2>/dev/null || true
# This should create conflicts in main.rs and config.toml

# ── Add unstaged changes to other files ─────────────────────────────────────

cat > README.md <<'EOF'
# Demo Project

A small demo project for testing stage-rs.

## Features
- Basic math operations
- Configuration file
- Multiple file types
- Syntax highlighting demo

## Getting Started
Run `cargo run` to start the application.
EOF

# Modify data.json (unstaged)
cat > data.json <<'EOF'
{
    "name": "test-project",
    "version": "2.0.0",
    "entries": [
        { "id": 1, "label": "alpha", "active": true },
        { "id": 2, "label": "beta", "active": false },
        { "id": 3, "label": "gamma", "active": true },
        { "id": 4, "label": "delta", "active": true }
    ],
    "metadata": {
        "created": "2024-01-01",
        "updated": "2024-06-15"
    }
}
EOF

# ── Stage some changes but not others ───────────────────────────────────────

# Stage README but leave data.json unstaged
git add README.md

# ── Add a new untracked file ────────────────────────────────────────────────

cat > notes.txt <<'EOF'
TODO:
- Add error handling
- Write more tests
- Set up CI/CD pipeline
- Review PR #42
EOF

# ── Add a partially staged file ─────────────────────────────────────────────
# Modify lib.rs, stage it, then modify again

cat > lib.rs <<'EOF'
/// Add two numbers.
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Subtract b from a.
pub fn subtract(a: i32, b: i32) -> i32 {
    a - b
}

/// Multiply two numbers.
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

/// Divide a by b, returning None if b is zero.
pub fn divide(a: i32, b: i32) -> Option<i32> {
    if b == 0 {
        None
    } else {
        Some(a / b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
    }

    #[test]
    fn test_subtract() {
        assert_eq!(subtract(5, 3), 2);
    }

    #[test]
    fn test_multiply() {
        assert_eq!(multiply(3, 4), 12);
    }
}
EOF

git add lib.rs

# Now modify lib.rs again (so it has both staged and unstaged changes)
cat >> lib.rs <<'EOF'

/// Compute the remainder of a divided by b.
pub fn modulo(a: i32, b: i32) -> Option<i32> {
    if b == 0 {
        None
    } else {
        Some(a % b)
    }
}
EOF

# ── Add a file with long lines for horizontal scroll testing ────────────────

cat > wide.rs <<'EOF'
fn example_with_very_long_lines() {
    let short = 1;
    let this_is_a_variable_with_a_really_long_name_that_goes_on_and_on_and_keeps_going_until_it_gets_ridiculous = "this is a string value that is also quite long and will definitely cause horizontal scrolling in most terminal windows";
    println!("short = {short}, long = {this_is_a_variable_with_a_really_long_name_that_goes_on_and_on_and_keeps_going_until_it_gets_ridiculous}");
}
EOF

# ── Summary ─────────────────────────────────────────────────────────────────

echo ""
echo "=== Test repo created at: $TARGET ==="
echo ""
echo "File states:"
echo "  CONFLICT  main.rs, config.toml  (merge conflict from feature-refactor)"
echo "  STAGED    README.md             (staged modification)"
echo "  STAGED+M  lib.rs                (staged changes + further unstaged edits)"
echo "  MODIFIED  data.json             (unstaged modification)"
echo "  UNTRACKED notes.txt, wide.rs    (new files)"
echo "  CLEAN     style.css, Makefile, src/utils.py"
echo ""
echo "Branches:"
echo "  master           — active branch with merge conflict in progress"
echo "  feature-refactor — branch used for the merge conflict"
echo "  rebase-test      — 2 commits that conflict with master (lib.rs, data.json)"
echo "                     Use: resolve merge, commit, then rebase rebase-test onto master"
echo ""
echo "Run:  cargo run -- $TARGET"
