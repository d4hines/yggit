use crate::{
    core::{push_from_notes, save_note, Note},
    git::{EnhancedCommit, Git},
    parser::{commits_to_string, Commit as ParsedCommit},
};
use clap::Args;
use std::collections::HashMap;

#[derive(Debug, Args)]
pub struct Push {
    /// Skip GitHub PR creation and management
    #[arg(long)]
    pub no_pr: bool,
}

const COMMENTS: &str = r#"
# Here is how to use yggit
# 
# Commands:
# -> <branch>                    add a branch to the above commit
# -> <origin>:<branch>           add a branch to the above commit with custom origin
# -> <branch> => <parent_branch> add a branch that branches from <parent_branch>
# 
# DAG Examples:
# -> feature-1            (branches from previous commit or main if first)
# -> feature-2 => main    (branches from main)
# -> feature-3            (branches from feature-2, the previous branch)
# 
# What happens next?
#  - All branches are pushed on origin, except if you specified a custom origin
#  - Branches with => syntax create proper Git parent relationships (DAG structure)
#
# It's not a rebase, you can't edit commits nor reorder them
"#;

impl Push {
    pub fn execute(&self, git: Git) -> Result<(), ()> {
        // Step 1: Capture the current state (before editing)
        let before_commits = git.list_commits();
        let before_state = extract_branch_state(&before_commits);

        let output = commits_to_string(before_commits);

        let file_path = "/tmp/yggit";

        let output = format!("{}\n{}", output, COMMENTS);
        std::fs::write(file_path, output).map_err(|_| println!("cannot write file to disk"))?;

        let content = git.edit_file(file_path)?;

        // Get the actual main branch name (main or master)
        let main_branch_name = git
            .main_branch()
            .and_then(|branch| branch.name().ok().flatten().map(|s| s.to_string()))
            .unwrap_or_else(|| "main".to_string());

        let after_commits = crate::parser::instruction_from_string_with_main_branch(
            content,
            main_branch_name.clone(),
        )
        .ok_or_else(|| {
            println!("Cannot parse instructions");
        })?;

        // Step 2: Extract the new state (after editing)
        let after_state = extract_branch_state_from_parsed(&after_commits);

        save_note(&git, after_commits);

        push_from_notes(&git);

        // Step 3: Handle GitHub PR integration (unless --no-pr flag is used)
        if !self.no_pr {
            handle_github_integration(&before_state, &after_state, &main_branch_name)?;
        } else {
            println!("⏭️  Skipping GitHub PR integration (--no-pr flag used)");
        }

        Ok(())
    }
}

/// Represents the state of a branch for PR management
#[derive(Debug, Clone, PartialEq)]
struct BranchState {
    branch: String,
    target_branch: String,
    origin: Option<String>,
    commit_title: Option<String>,
}

/// Extract branch states from EnhancedCommits (with notes)
fn extract_branch_state(commits: &[EnhancedCommit<Note>]) -> HashMap<String, BranchState> {
    let mut states = HashMap::new();

    for commit in commits {
        if let Some(note) = &commit.note {
            if let Some(push) = &note.push {
                let target_branch = push
                    .parent_branch
                    .as_ref()
                    .unwrap_or(&"main".to_string())
                    .clone();

                let state = BranchState {
                    branch: push.branch.clone(),
                    target_branch,
                    origin: push.origin.clone(),
                    commit_title: Some(commit.title.clone()),
                };

                states.insert(push.branch.clone(), state);
            }
        }
    }

    states
}

/// Extract branch states from parsed commits (before notes are saved)
fn extract_branch_state_from_parsed(commits: &[ParsedCommit]) -> HashMap<String, BranchState> {
    let mut states = HashMap::new();

    for commit in commits {
        if let Some(target) = &commit.target {
            let target_branch = target
                .parent_branch
                .as_ref()
                .unwrap_or(&"main".to_string())
                .clone();

            let state = BranchState {
                branch: target.branch.clone(),
                target_branch,
                origin: target.origin.clone(),
                commit_title: Some(commit.title.clone()),
            };

            states.insert(target.branch.clone(), state);
        }
    }

    states
}

/// Handle GitHub PR integration by comparing before/after states
fn handle_github_integration(
    before_state: &HashMap<String, BranchState>,
    after_state: &HashMap<String, BranchState>,
    main_branch_name: &str,
) -> Result<(), ()> {
    // Check if gh CLI is available
    if !is_gh_available() {
        println!("📝 GitHub CLI (gh) not found. Skipping PR integration.");
        println!("   Install gh CLI for automatic PR management: https://cli.github.com/");
        return Ok(());
    }

    println!("🔗 Managing GitHub Pull Requests...");

    // Handle new branches and target changes
    for (branch_name, after_branch) in after_state {
        if !before_state.contains_key(branch_name) {
            // New branch - create PR
            println!("🆕 New branch detected: {}", branch_name);
            create_pull_request(after_branch, main_branch_name)?;
        } else {
            // Existing branch - check if target changed
            let before_branch = &before_state[branch_name];
            if before_branch.target_branch != after_branch.target_branch {
                // Target changed - update PR
                println!(
                    "🔄 Target changed for {}: {} -> {}",
                    branch_name, before_branch.target_branch, after_branch.target_branch
                );
                update_pull_request_base(after_branch, &before_branch.target_branch)?;
            } else {
                // Check if PR exists, create if missing
                if !pr_exists(branch_name)? {
                    println!("📝 No PR found for existing branch: {}", branch_name);
                    create_pull_request(after_branch, main_branch_name)?;
                }
            }
        }
    }

    // Find removed branches (in before but not in after)
    for (branch_name, _before_branch) in before_state {
        if !after_state.contains_key(branch_name) {
            println!("ℹ️  Branch '{}' removed. PR will remain open.", branch_name);
        }
    }

    Ok(())
}

/// Check if gh CLI is available
fn is_gh_available() -> bool {
    std::process::Command::new("gh")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Check if a PR exists for the given branch
fn pr_exists(branch_name: &str) -> Result<bool, ()> {
    let mut cmd = std::process::Command::new("gh");
    cmd.args(["pr", "list", "--head", branch_name, "--json", "number"]);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                // If the JSON output is "[]", no PRs exist for this branch
                let exists = !stdout.trim().eq("[]");
                println!("🔍 Debug - PR exists for {}: {}", branch_name, exists);
                Ok(exists)
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                println!(
                    "⚠️  Warning: Could not check PR status for {}: {}",
                    branch_name, stderr
                );
                // If we can't check, assume it doesn't exist and try to create it
                Ok(false)
            }
        }
        Err(e) => {
            println!("❌ Error checking PR status: {}", e);
            Err(())
        }
    }
}

/// Create a new pull request using gh CLI
fn create_pull_request(branch_state: &BranchState, _main_branch_name: &str) -> Result<(), ()> {
    let target = &branch_state.target_branch;

    // Use commit title as PR title, fallback to branch name
    let pr_title = branch_state
        .commit_title
        .as_ref()
        .unwrap_or(&branch_state.branch);

    println!(
        "📝 Creating PR: {} → {} (\"{}\")",
        branch_state.branch, target, pr_title
    );

    let mut cmd = std::process::Command::new("gh");
    cmd.args([
        "pr",
        "create",
        "--head",
        &branch_state.branch,
        "--base",
        target,
        "--title",
        pr_title,
        "--body",
        &format!(
            "Auto-created PR for branch `{}` targeting `{}`\n\n🤖 Created by yggit",
            branch_state.branch, target
        ),
    ]);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().is_empty() {
                    println!("✅ Created PR: {}", stdout.trim());
                } else {
                    println!("✅ Created PR for {}", branch_state.branch);
                }
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if stderr.contains("already exists") {
                    println!("ℹ️  PR for {} already exists", branch_state.branch);
                } else {
                    println!(
                        "❌ Failed to create PR for {}: {}",
                        branch_state.branch, stderr
                    );
                }
            }
        }
        Err(e) => {
            println!("❌ Error running gh CLI: {}", e);
            return Err(());
        }
    }

    Ok(())
}

/// Update the base branch of an existing pull request
fn update_pull_request_base(branch_state: &BranchState, old_target: &str) -> Result<(), ()> {
    let new_target = &branch_state.target_branch;

    println!(
        "🔄 Updating PR base: {} ({} → {})",
        branch_state.branch, old_target, new_target
    );

    let mut cmd = std::process::Command::new("gh");
    cmd.args(["pr", "edit", &branch_state.branch, "--base", new_target]);

    match cmd.output() {
        Ok(output) => {
            if output.status.success() {
                println!("✅ Updated PR base for {}", branch_state.branch);
            } else {
                let error = String::from_utf8_lossy(&output.stderr);
                if error.contains("not found") {
                    println!(
                        "ℹ️  No existing PR found for {}. Creating new PR...",
                        branch_state.branch
                    );
                    create_pull_request(branch_state, new_target)?;
                } else {
                    println!(
                        "❌ Failed to update PR for {}: {}",
                        branch_state.branch, error
                    );
                }
            }
        }
        Err(e) => {
            println!("❌ Error running gh CLI: {}", e);
            return Err(());
        }
    }

    Ok(())
}
