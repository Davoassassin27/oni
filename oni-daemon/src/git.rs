use std::path::{Path, PathBuf};
use std::process::Output;

use anyhow::{Context, Result, bail};
use tokio::process::Command;

use crate::model::{DiffFilePatch, DiffPatch, DiffSummary, FileChange};

async fn run_git(dir: &Path, args: &[&str]) -> Result<Output> {
    let mut command = Command::new("git");
    command
        .current_dir(dir)
        .args(args)
        .kill_on_drop(true)
        .env("GIT_TERMINAL_PROMPT", "0");
    command
        .output()
        .await
        .with_context(|| format!("cannot run git {}", args.join(" ")))
}

async fn run_git_checked(dir: &Path, args: &[&str]) -> Result<String> {
    let output = run_git(dir, args).await?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "git {} failed: {}",
            args.join(" "),
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            }
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Validate a repository and return its canonical top-level path.
pub async fn validate_repo(path: &Path, base_branch: &str) -> Result<(PathBuf, String)> {
    let canonical = tokio::fs::canonicalize(path)
        .await
        .with_context(|| format!("repository path does not exist: {}", path.display()))?;

    if !canonical.is_dir() {
        bail!(
            "repository path is not a directory: {}",
            canonical.display()
        );
    }

    let top_level = run_git_checked(&canonical, &["rev-parse", "--show-toplevel"]).await?;
    let top_level = PathBuf::from(top_level);

    run_git_checked(&top_level, &["rev-parse", "--verify", "HEAD"])
        .await
        .context("repository has no commits yet")?;

    let base = base_branch.trim();
    if base.is_empty() {
        bail!("base branch cannot be empty");
    }

    let reference = format!("refs/heads/{base}");
    let output = run_git(&top_level, &["show-ref", "--verify", "--quiet", &reference]).await?;
    if !output.status.success() {
        bail!("base branch '{base}' does not exist in this repository");
    }

    Ok((top_level, base.to_string()))
}

pub async fn current_branch(repo: &Path) -> Result<String> {
    run_git_checked(repo, &["symbolic-ref", "--short", "HEAD"]).await
}

pub async fn is_clean(repo: &Path) -> Result<bool> {
    let output = run_git_checked(repo, &["status", "--porcelain"]).await?;
    Ok(output.is_empty())
}

pub async fn create_worktree(
    repo: &Path,
    base_branch: &str,
    branch: &str,
    worktree: &Path,
) -> Result<()> {
    if tokio::fs::try_exists(worktree).await.unwrap_or(false) {
        bail!("worktree path already exists: {}", worktree.display());
    }
    if let Some(parent) = worktree.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("cannot create {}", parent.display()))?;
    }

    let worktree_arg = worktree.to_string_lossy().to_string();
    run_git_checked(
        repo,
        &["worktree", "add", "-b", branch, &worktree_arg, base_branch],
    )
    .await?;
    Ok(())
}

pub async fn remove_worktree(repo: &Path, worktree: &Path) -> Result<()> {
    if !tokio::fs::try_exists(worktree).await.unwrap_or(false) {
        let _ = run_git(repo, &["worktree", "prune"]).await;
        return Ok(());
    }

    let worktree_arg = worktree.to_string_lossy().to_string();
    run_git_checked(repo, &["worktree", "remove", "--force", &worktree_arg]).await?;
    let _ = run_git(repo, &["worktree", "prune"]).await;
    Ok(())
}

pub async fn delete_branch(repo: &Path, branch: &str) -> Result<()> {
    run_git_checked(repo, &["branch", "-D", branch]).await?;
    Ok(())
}

pub async fn merge_branch(repo: &Path, base_branch: &str, branch: &str) -> Result<()> {
    let checked_out = current_branch(repo).await?;
    if checked_out != base_branch {
        bail!("cannot merge: '{base_branch}' must be checked out (currently on '{checked_out}')");
    }
    if !is_clean(repo).await? {
        bail!("cannot merge: the repository has uncommitted changes. Commit or stash them first.");
    }

    let message = format!("oni: merge {branch}");
    let output = run_git(
        repo,
        &["merge", "--no-ff", branch, "-m", &message, "--no-edit"],
    )
    .await?;

    if !output.status.success() {
        let _ = run_git(repo, &["merge", "--abort"]).await;
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "merge failed and was aborted: {}",
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            }
        );
    }

    Ok(())
}

/// Commit everything the agent left in the worktree. Returns true when a
/// commit was created. The commit is attributed to Oni, not to the human.
pub async fn commit_all(worktree: &Path, title: &str) -> Result<bool> {
    let status = run_git_checked(worktree, &["status", "--porcelain"]).await?;
    if status.is_empty() {
        return Ok(false);
    }

    run_git_checked(worktree, &["add", "-A"]).await?;
    let message = format!("oni: {title}");
    run_git_checked(
        worktree,
        &[
            "-c",
            "user.name=Oni",
            "-c",
            "user.email=oni@localhost",
            "commit",
            "-m",
            &message,
        ],
    )
    .await?;
    Ok(true)
}

pub async fn diff_summary(repo: &Path, base_branch: &str, branch: &str) -> Result<DiffSummary> {
    let range = format!("{base_branch}...{branch}");
    let stat = run_git_checked(repo, &["diff", "--stat", "--no-color", &range]).await?;
    let numstat = run_git_checked(repo, &["diff", "--numstat", "--no-color", &range]).await?;
    let (files, insertions, deletions) = parse_numstat(&numstat);

    Ok(DiffSummary {
        stat,
        files,
        insertions,
        deletions,
    })
}

const MAX_PATCH_BYTES: usize = 512 * 1024;

pub async fn diff_patch(repo: &Path, base_branch: &str, branch: &str) -> Result<DiffPatch> {
    let range = format!("{base_branch}...{branch}");
    let output = run_git(
        repo,
        &["diff", "--no-color", "--unified=3", "--no-ext-diff", &range],
    )
    .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        bail!(
            "git diff failed: {}",
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            }
        );
    }

    let bytes = output.stdout;
    let truncated = bytes.len() > MAX_PATCH_BYTES;
    let slice = if truncated {
        let cut = bytes[..MAX_PATCH_BYTES]
            .iter()
            .rposition(|byte| *byte == b'\n')
            .map(|position| position + 1)
            .unwrap_or(MAX_PATCH_BYTES);
        &bytes[..cut]
    } else {
        &bytes
    };

    let raw = String::from_utf8_lossy(slice);
    Ok(DiffPatch {
        files: parse_patch(&raw),
        truncated,
    })
}

fn parse_patch(raw: &str) -> Vec<DiffFilePatch> {
    let mut files: Vec<DiffFilePatch> = Vec::new();
    let mut current: Option<DiffFilePatch> = None;

    for line in raw.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            if let Some(file) = current.take() {
                files.push(file);
            }
            current = Some(DiffFilePatch {
                path: parse_diff_git_path(line).unwrap_or("unknown").to_string(),
                patch: String::new(),
                insertions: 0,
                deletions: 0,
            });
        }

        let Some(file) = current.as_mut() else {
            continue;
        };

        if let Some(path) = line.strip_prefix("+++ b/") {
            file.path = path.trim().to_string();
        }
        if line.starts_with('+') && !line.starts_with("+++") {
            file.insertions += 1;
        }
        if line.starts_with('-') && !line.starts_with("---") {
            file.deletions += 1;
        }
        file.patch.push_str(line);
    }

    if let Some(file) = current {
        files.push(file);
    }

    files
}

fn parse_diff_git_path(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("diff --git ")?;
    let marker = rest.find(" b/")?;
    Some(rest[marker + 3..].trim())
}

fn parse_numstat(numstat: &str) -> (Vec<FileChange>, u64, u64) {
    let mut files = Vec::new();
    let mut insertions = 0u64;
    let mut deletions = 0u64;

    for line in numstat.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };

        let added = added.parse::<u64>().unwrap_or(0);
        let removed = removed.parse::<u64>().unwrap_or(0);
        insertions += added;
        deletions += removed;
        files.push(FileChange {
            path: path.to_string(),
            insertions: added,
            deletions: removed,
        });
    }

    (files, insertions, deletions)
}

pub fn slugify(input: &str, max_len: usize) -> String {
    let mut slug = String::with_capacity(input.len());
    let mut last_was_dash = false;

    for character in input.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
        if slug.len() >= max_len {
            break;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }
    if slug.is_empty() { "task".into() } else { slug }
}

pub fn branch_name(title: &str, task_id: uuid::Uuid) -> String {
    let short = task_id.simple().to_string();
    let short = &short[..8];
    format!("oni/{}-{short}", slugify(title, 36))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugify_keeps_it_branch_safe() {
        assert_eq!(
            slugify("Fix the flaky auth test!", 36),
            "fix-the-flaky-auth-test"
        );
        assert_eq!(slugify("  ", 36), "task");
        assert_eq!(slugify("a/b\\c", 36), "a-b-c");
        assert!(slugify("x".repeat(100).as_str(), 20).len() <= 20);
    }

    #[test]
    fn branch_name_is_prefixed_and_unique() {
        let first = branch_name("Add rate limiting", uuid::Uuid::nil());
        let second = branch_name("Add rate limiting", uuid::Uuid::max());
        assert!(first.starts_with("oni/add-rate-limiting-"));
        assert_ne!(first, second);
    }

    #[test]
    fn numstat_parses_binary_and_text() {
        let (files, insertions, deletions) =
            parse_numstat("3\t1\tsrc/main.rs\n12\t0\tREADME.md\n-\t-\tassets/logo.png\n");
        assert_eq!(files.len(), 3);
        assert_eq!(insertions, 15);
        assert_eq!(deletions, 1);
        assert_eq!(files[2].path, "assets/logo.png");
        assert_eq!(files[2].insertions, 0);
    }

    #[test]
    fn patch_parser_splits_files_and_counts_lines() {
        let raw = "\
diff --git a/src/lib.rs b/src/lib.rs
index 1111111..2222222 100644
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1,3 +1,4 @@
 fn main() {
-    old();
+    new();
+    also_new();
 }
diff --git a/README.md b/README.md
index 3333333..4444444 100644
--- a/README.md
+++ b/README.md
@@ -1 +1,2 @@
 hello
+world
";
        let files = parse_patch(raw);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/lib.rs");
        assert_eq!(files[0].insertions, 2);
        assert_eq!(files[0].deletions, 1);
        assert_eq!(files[1].path, "README.md");
        assert_eq!(files[1].insertions, 1);
        assert_eq!(files[1].deletions, 0);
        assert!(files[0].patch.contains("@@ -1,3 +1,4 @@"));
    }
}
