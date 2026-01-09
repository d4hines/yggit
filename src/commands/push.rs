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

    /// Skip pushing branches to remote
    #[arg(long)]
    pub no_push: bool,
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
# In-Band Commands (place at the top of the file):
# ABORT                   abort the operation
# NO_PR                   skip GitHub PR creation (same as --no-pr)
# NO_PUSH                 skip pushing branches to remote (same as --no-push)
#
# What happens next?
#  - All branches are pushed on origin, except if you specified a custom origin
#  - Branches with => syntax create proper Git parent relationships (DAG structure)
#
# It's not a rebase, you can't edit commits nor reorder them
"#;

/// In-band commands parsed from the file content
#[derive(Debug, Default)]
struct InBandCommands {
    abort: bool,
    no_pr: bool,
    no_push: bool,
}

/// Parse in-band commands from the content and return cleaned content
fn parse_in_band_commands(content: &str) -> (String, InBandCommands) {
    let mut commands = InBandCommands::default();
    let mut cleaned_lines = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        match trimmed {
            "ABORT" => commands.abort = true,
            "NO_PR" => commands.no_pr = true,
            "NO_PUSH" => commands.no_push = true,
            _ => cleaned_lines.push(line),
        }
    }

    (cleaned_lines.join("\n"), commands)
}

impl Push {
    pub fn execute(&self, git: Git) -> Result<(), ()> {
        // Step 1: Capture the current state (before editing)
        let before_commits = git.list_commits();
        let before_state = extract_branch_state(&before_commits);

        let output = commits_to_string(before_commits.clone());

        let file_path = "/tmp/yggit";

        let output = format!("{}\n{}", output, COMMENTS);
        std::fs::write(file_path, output).map_err(|_| println!("cannot write file to disk"))?;

        let content = git.edit_file(file_path)?;

        // Parse in-band commands from the content
        let (cleaned_content, in_band_commands) = parse_in_band_commands(&content);

        // Handle ABORT command
        if in_band_commands.abort {
            println!("🛑 ABORT command detected. Operation cancelled.");
            return Err(());
        }

        // Override flags with in-band commands
        let no_pr = self.no_pr || in_band_commands.no_pr;
        let no_push = self.no_push || in_band_commands.no_push;

        // Get the actual main branch name (main or master)
        let main_branch_name = git
            .main_branch()
            .and_then(|branch| branch.name().ok().flatten().map(|s| s.to_string()))
            .unwrap_or_else(|| "main".to_string());

        let after_commits = crate::parser::instruction_from_string_with_main_branch(
            cleaned_content,
            main_branch_name.clone(),
        )
        .ok_or_else(|| {
            println!("Cannot parse instructions");
        })?;

        // Step 2: Extract the new state (after editing)
        let after_state =
            extract_branch_state_from_parsed(&after_commits)
                .iter()
                .map(|(key, x)| (key.clone(), BranchState {
                    branch: x.branch.clone(),
                    target_branch: x.target_branch.clone(),
                    origin: x.origin.clone(),
                    commit_title: x.commit_title.clone(),
                    commit_description: {
                        // Find the corresponding commit in after_commits to get its hash
                        after_commits.iter()
                            .find(|commit| commit.target.as_ref()
                                .map(|t| &t.branch) == Some(&x.branch))
                            .and_then(|after_commit| {
                                // Find the same commit (by hash) in before_commits to get its description
                                before_commits.iter()
                                    .find(|before_commit| before_commit.id == after_commit.hash)
                                    .and_then(|before_commit| before_commit.description.clone())
                            })
                    },
                })).collect();

        save_note(&git, after_commits);

        // Step 2.5: Push branches (unless --no-push flag or NO_PUSH command is used)
        if !no_push {
            push_from_notes(&git);
        } else {
            println!("⏭️  Skipping push to remote (--no-push flag or NO_PUSH command used)");
        }

        // Step 3: Handle GitHub PR integration (unless --no-pr flag or NO_PR command is used)
        if !no_pr {
            handle_github_integration(&before_state, &after_state, &main_branch_name)?;
        } else {
            println!("⏭️  Skipping GitHub PR integration (--no-pr flag or NO_PR command used)");
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
    commit_title: String,
    commit_description: Option<String>,
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
                    commit_title: commit.title.clone(),
                    commit_description: commit.description.clone(),
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
                commit_title: commit.title.clone(),
                commit_description: None,
            };

            states.insert(target.branch.clone(), state);
        }
    }

    states
}

/// Handle GitHub PR integration by comparing desired state with actual GitHub state
fn handle_github_integration(
    _before_state: &HashMap<String, BranchState>,
    after_state: &HashMap<String, BranchState>,
    _main_branch_name: &str,
) -> Result<(), ()> {
    handle_github_integration_with_ops(&RealGitHubOps, after_state)
}

/// Handle GitHub PR integration with injectable GitHub operations (for testing)
fn handle_github_integration_with_ops(
    gh_ops: &dyn GitHubOperations,
    desired_state: &HashMap<String, BranchState>,
) -> Result<(), ()> {
    // Check if gh CLI is available
    if !gh_ops.is_available() {
        println!("📝 GitHub CLI (gh) not found. Skipping PR integration.");
        println!("   Install gh CLI for automatic PR management: https://cli.github.com/");
        return Ok(());
    }

    println!("🔗 Managing GitHub Pull Requests...");

    // For each desired branch, compare with actual GitHub state
    for (branch_name, desired_branch) in desired_state {
        match gh_ops.get_pr_info(branch_name)? {
            None => {
                // No PR exists - create it
                println!("📝 No PR found for '{}', creating...", branch_name);
                gh_ops.create_pr(desired_branch)?;
            }
            Some(pr_info) => {
                if pr_info.base_branch != desired_branch.target_branch {
                    // PR exists but base branch is different - update it
                    println!(
                        "🔄 Updating PR base for '{}': {} → {}",
                        branch_name, pr_info.base_branch, desired_branch.target_branch
                    );
                    gh_ops.update_pr_base(desired_branch, &pr_info.base_branch)?;
                } else {
                    // PR exists with correct base - nothing to do
                    println!("✓ PR for '{}' already exists with correct base", branch_name);
                }
            }
        }
    }

    Ok(())
}

/// Information about an existing PR
#[derive(Debug, Clone, PartialEq)]
struct PrInfo {
    base_branch: String,
}

/// Trait for GitHub operations, allowing for testing with mock implementations
trait GitHubOperations {
    fn is_available(&self) -> bool;
    fn get_pr_info(&self, branch_name: &str) -> Result<Option<PrInfo>, ()>;
    fn create_pr(&self, branch_state: &BranchState) -> Result<(), ()>;
    fn update_pr_base(&self, branch_state: &BranchState, old_base: &str) -> Result<(), ()>;
}

/// Real implementation using gh CLI
struct RealGitHubOps;

impl GitHubOperations for RealGitHubOps {
    fn is_available(&self) -> bool {
        std::process::Command::new("gh")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    fn get_pr_info(&self, branch_name: &str) -> Result<Option<PrInfo>, ()> {
        let mut cmd = std::process::Command::new("gh");
        cmd.args([
            "pr",
            "list",
            "--head",
            branch_name,
            "--json",
            "baseRefName",
            "--limit",
            "1",
        ]);

        match cmd.output() {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    // Parse JSON: [{"baseRefName": "main"}] or []
                    if stdout.trim() == "[]" {
                        Ok(None)
                    } else {
                        // Simple JSON parsing for baseRefName
                        if let Some(base_start) = stdout.find("\"baseRefName\":") {
                            if let Some(value_start) = stdout[base_start..].find('"') {
                                let value_offset = base_start + value_start + 1;
                                if let Some(value_end) = stdout[value_offset..].find('"') {
                                    let base_branch =
                                        stdout[value_offset..value_offset + value_end].to_string();
                                    return Ok(Some(PrInfo { base_branch }));
                                }
                            }
                        }
                        println!("⚠️  Warning: Could not parse PR info for {}", branch_name);
                        Ok(None)
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    println!(
                        "⚠️  Warning: Could not get PR info for {}: {}",
                        branch_name, stderr
                    );
                    Ok(None)
                }
            }
            Err(e) => {
                println!("❌ Error getting PR info: {}", e);
                Err(())
            }
        }
    }

    fn create_pr(&self, branch_state: &BranchState) -> Result<(), ()> {
        create_pull_request(branch_state, &branch_state.target_branch)
    }

    fn update_pr_base(&self, branch_state: &BranchState, old_base: &str) -> Result<(), ()> {
        update_pull_request_base(branch_state, old_base)
    }
}

/// Create a new pull request using gh CLI
fn create_pull_request(branch_state: &BranchState, _main_branch_name: &str) -> Result<(), ()> {
    let target = &branch_state.target_branch;

    println!(
        "📝 Creating PR: {} → {} (\"{}\")",
        branch_state.branch, target, branch_state.branch
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
        &branch_state.commit_title,
        "--body",
        &format!(
            "{}`\n\n🤖 Created by yggit",
            branch_state
                .commit_description
                .clone()
                .unwrap_or_default()
                .trim()
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// Mock GitHub operations for testing
    struct MockGitHubOps {
        available: bool,
        pr_state: RefCell<HashMap<String, Option<PrInfo>>>,
        created_prs: RefCell<Vec<String>>,
        updated_prs: RefCell<Vec<(String, String, String)>>, // (branch, old_base, new_base)
    }

    impl MockGitHubOps {
        fn new(available: bool) -> Self {
            Self {
                available,
                pr_state: RefCell::new(HashMap::new()),
                created_prs: RefCell::new(Vec::new()),
                updated_prs: RefCell::new(Vec::new()),
            }
        }

        fn with_pr(self, branch: &str, base: &str) -> Self {
            self.pr_state.borrow_mut().insert(
                branch.to_string(),
                Some(PrInfo {
                    base_branch: base.to_string(),
                }),
            );
            self
        }

        fn with_no_pr(self, branch: &str) -> Self {
            self.pr_state
                .borrow_mut()
                .insert(branch.to_string(), None);
            self
        }
    }

    impl GitHubOperations for MockGitHubOps {
        fn is_available(&self) -> bool {
            self.available
        }

        fn get_pr_info(&self, branch_name: &str) -> Result<Option<PrInfo>, ()> {
            Ok(self
                .pr_state
                .borrow()
                .get(branch_name)
                .cloned()
                .unwrap_or(None))
        }

        fn create_pr(&self, branch_state: &BranchState) -> Result<(), ()> {
            self.created_prs
                .borrow_mut()
                .push(branch_state.branch.clone());
            Ok(())
        }

        fn update_pr_base(&self, branch_state: &BranchState, old_base: &str) -> Result<(), ()> {
            self.updated_prs.borrow_mut().push((
                branch_state.branch.clone(),
                old_base.to_string(),
                branch_state.target_branch.clone(),
            ));
            Ok(())
        }
    }

    #[test]
    fn test_github_integration_no_pr_exists_creates_pr() {
        let mock = MockGitHubOps::new(true).with_no_pr("feature-1");

        let mut desired_state = HashMap::new();
        desired_state.insert(
            "feature-1".to_string(),
            BranchState {
                branch: "feature-1".to_string(),
                target_branch: "main".to_string(),
                origin: None,
                commit_title: "Add feature 1".to_string(),
                commit_description: None,
            },
        );

        let result = handle_github_integration_with_ops(&mock, &desired_state);
        assert!(result.is_ok());
        assert_eq!(mock.created_prs.borrow().len(), 1);
        assert_eq!(mock.created_prs.borrow()[0], "feature-1");
        assert_eq!(mock.updated_prs.borrow().len(), 0);
    }

    #[test]
    fn test_github_integration_pr_exists_same_base_no_action() {
        let mock = MockGitHubOps::new(true).with_pr("feature-1", "main");

        let mut desired_state = HashMap::new();
        desired_state.insert(
            "feature-1".to_string(),
            BranchState {
                branch: "feature-1".to_string(),
                target_branch: "main".to_string(),
                origin: None,
                commit_title: "Add feature 1".to_string(),
                commit_description: None,
            },
        );

        let result = handle_github_integration_with_ops(&mock, &desired_state);
        assert!(result.is_ok());
        assert_eq!(mock.created_prs.borrow().len(), 0);
        assert_eq!(mock.updated_prs.borrow().len(), 0);
    }

    #[test]
    fn test_github_integration_pr_exists_different_base_updates() {
        let mock = MockGitHubOps::new(true).with_pr("feature-1", "develop");

        let mut desired_state = HashMap::new();
        desired_state.insert(
            "feature-1".to_string(),
            BranchState {
                branch: "feature-1".to_string(),
                target_branch: "main".to_string(),
                origin: None,
                commit_title: "Add feature 1".to_string(),
                commit_description: None,
            },
        );

        let result = handle_github_integration_with_ops(&mock, &desired_state);
        assert!(result.is_ok());
        assert_eq!(mock.created_prs.borrow().len(), 0);
        assert_eq!(mock.updated_prs.borrow().len(), 1);
        assert_eq!(
            mock.updated_prs.borrow()[0],
            ("feature-1".to_string(), "develop".to_string(), "main".to_string())
        );
    }

    #[test]
    fn test_github_integration_lost_notes_scenario() {
        // Simulates the scenario where git notes were lost:
        // - PRs exist on GitHub for feature-1 (base: main) and feature-2 (base: feature-1)
        // - User re-adds annotations
        // - Should detect existing PRs and not try to recreate them
        let mock = MockGitHubOps::new(true)
            .with_pr("feature-1", "main")
            .with_pr("feature-2", "feature-1");

        let mut desired_state = HashMap::new();
        desired_state.insert(
            "feature-1".to_string(),
            BranchState {
                branch: "feature-1".to_string(),
                target_branch: "main".to_string(),
                origin: None,
                commit_title: "Add feature 1".to_string(),
                commit_description: None,
            },
        );
        desired_state.insert(
            "feature-2".to_string(),
            BranchState {
                branch: "feature-2".to_string(),
                target_branch: "feature-1".to_string(),
                origin: None,
                commit_title: "Add feature 2".to_string(),
                commit_description: None,
            },
        );

        let result = handle_github_integration_with_ops(&mock, &desired_state);
        assert!(result.is_ok());
        // Should not create any PRs since they already exist with correct bases
        assert_eq!(mock.created_prs.borrow().len(), 0);
        assert_eq!(mock.updated_prs.borrow().len(), 0);
    }

    #[test]
    fn test_github_integration_gh_not_available() {
        let mock = MockGitHubOps::new(false);
        let desired_state = HashMap::new();

        let result = handle_github_integration_with_ops(&mock, &desired_state);
        assert!(result.is_ok());
        // Should exit early without any operations
        assert_eq!(mock.created_prs.borrow().len(), 0);
        assert_eq!(mock.updated_prs.borrow().len(), 0);
    }

    #[test]
    fn test_parse_in_band_commands_abort() {
        let content = "ABORT\n8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit\n-> branch";
        let (cleaned, commands) = parse_in_band_commands(content);
        assert!(commands.abort);
        assert!(!commands.no_pr);
        assert!(!commands.no_push);
        assert!(!cleaned.contains("ABORT"));
        assert!(cleaned.contains("8c14734b80ff0ffb93caefc85553c7c5b05cca1e"));
    }

    #[test]
    fn test_parse_in_band_commands_no_pr() {
        let content = "NO_PR\n8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit\n-> branch";
        let (cleaned, commands) = parse_in_band_commands(content);
        assert!(!commands.abort);
        assert!(commands.no_pr);
        assert!(!commands.no_push);
        assert!(!cleaned.contains("NO_PR"));
        assert!(cleaned.contains("8c14734b80ff0ffb93caefc85553c7c5b05cca1e"));
    }

    #[test]
    fn test_parse_in_band_commands_no_push() {
        let content = "NO_PUSH\n8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit\n-> branch";
        let (cleaned, commands) = parse_in_band_commands(content);
        assert!(!commands.abort);
        assert!(!commands.no_pr);
        assert!(commands.no_push);
        assert!(!cleaned.contains("NO_PUSH"));
        assert!(cleaned.contains("8c14734b80ff0ffb93caefc85553c7c5b05cca1e"));
    }

    #[test]
    fn test_parse_in_band_commands_multiple() {
        let content = "NO_PR\nNO_PUSH\n8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit\n-> branch";
        let (cleaned, commands) = parse_in_band_commands(content);
        assert!(!commands.abort);
        assert!(commands.no_pr);
        assert!(commands.no_push);
        assert!(!cleaned.contains("NO_PR"));
        assert!(!cleaned.contains("NO_PUSH"));
        assert!(cleaned.contains("8c14734b80ff0ffb93caefc85553c7c5b05cca1e"));
    }

    #[test]
    fn test_parse_in_band_commands_with_whitespace() {
        let content = "  NO_PR  \n8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit\n-> branch";
        let (cleaned, commands) = parse_in_band_commands(content);
        assert!(commands.no_pr);
        assert!(!cleaned.contains("NO_PR"));
    }

    #[test]
    fn test_parse_in_band_commands_none() {
        let content = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit\n-> branch";
        let (cleaned, commands) = parse_in_band_commands(content);
        assert!(!commands.abort);
        assert!(!commands.no_pr);
        assert!(!commands.no_push);
        assert_eq!(cleaned, content);
    }

    #[test]
    fn test_parse_in_band_commands_preserves_comments() {
        let content = "NO_PR\n# This is a comment\n8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit";
        let (cleaned, commands) = parse_in_band_commands(content);
        assert!(commands.no_pr);
        assert!(cleaned.contains("# This is a comment"));
    }
}
