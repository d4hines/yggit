use crate::errors::{Result, YggitError};
use crate::github::{GitHubCli, BranchState};
use crate::git::EnhancedCommit;
use crate::core::Note;
use crate::parser::Commit as ParsedCommit;
use std::collections::HashMap;

pub struct GitHubIntegration<T: GitHubCli> {
    pub github_cli: T,
}

impl<T: GitHubCli> GitHubIntegration<T> {
    pub fn new(github_cli: T) -> Self {
        Self { github_cli }
    }
    
    pub fn handle_integration(
        &self,
        before_state: &HashMap<String, BranchState>,
        after_state: &HashMap<String, BranchState>,
        main_branch_name: &str,
    ) -> Result<()> {
        // Check if GitHub CLI is available
        if !self.github_cli.is_available()? {
            log::info!("📝 GitHub CLI (gh) not found. Skipping PR integration.");
            log::info!("   Install gh CLI for automatic PR management: https://cli.github.com/");
            return Ok(());
        }

        log::info!("🔗 Managing GitHub Pull Requests...");

        // Handle new branches and target changes
        for (branch_name, after_branch) in after_state {
            if !before_state.contains_key(branch_name) {
                // New branch - create PR
                log::info!("🆕 New branch detected: {}", branch_name);
                let branch_with_description = self.find_branch_with_description(after_branch, before_state);
                self.create_pull_request(&branch_with_description, main_branch_name)?;
            } else {
                // Existing branch - check if target changed
                let before_branch = &before_state[branch_name];
                if before_branch.target_branch != after_branch.target_branch {
                    // Target changed - update PR
                    log::info!("🔄 Target changed for {}: {} -> {}", 
                              branch_name, before_branch.target_branch, after_branch.target_branch);
                    self.update_pull_request_base(after_branch, &before_branch.target_branch)?;
                } else {
                    // Check if PR exists, create if missing
                    if !self.github_cli.pr_exists(branch_name)? {
                        log::info!("📝 No PR found for existing branch: {}", branch_name);
                        self.create_pull_request(before_branch, main_branch_name)?;
                    }
                }
            }
        }

        // Find removed branches (in before but not in after)
        for (branch_name, _before_branch) in before_state {
            if !after_state.contains_key(branch_name) {
                log::info!("ℹ️  Branch '{}' removed. PR will remain open.", branch_name);
            }
        }

        Ok(())
    }
    
    pub fn find_branch_with_description(
        &self,
        after_branch: &BranchState,
        before_state: &HashMap<String, BranchState>,
    ) -> BranchState {
        // Look for a branch in before_state with the same commit title
        for (_, before_branch) in before_state {
            if before_branch.commit_title == after_branch.commit_title {
                // Found a match, create a new BranchState with the description but other fields from after_branch
                return BranchState {
                    branch: after_branch.branch.clone(),
                    target_branch: after_branch.target_branch.clone(),
                    origin: after_branch.origin.clone(),
                    commit_title: after_branch.commit_title.clone(),
                    commit_description: before_branch.commit_description.clone(),
                };
            }
        }
        
        // No match found, return the original after_branch
        after_branch.clone()
    }
    
    fn create_pull_request(&self, branch_state: &BranchState, _main_branch_name: &str) -> Result<()> {
        let target = &branch_state.target_branch;
        
        // Use commit title as PR title, fallback to branch name
        let pr_title = branch_state.commit_title.as_ref()
            .unwrap_or(&branch_state.branch);
        
        let pr_body = format!("{}\n\n🤖 Created by yggit", 
                             branch_state.commit_description.as_ref()
                                 .unwrap_or(&String::new()));
        
        match self.github_cli.create_pr(&branch_state.branch, target, pr_title, &pr_body) {
            Ok(result) => {
                log::info!("✅ {}", result);
                Ok(())
            }
            Err(YggitError::GitHubCli(ref error)) if error.contains("already exists") => {
                log::info!("ℹ️  PR for {} already exists", branch_state.branch);
                Ok(())
            }
            Err(e) => {
                log::error!("❌ Failed to create PR for {}: {}", branch_state.branch, e);
                Err(e)
            }
        }
    }
    
    fn update_pull_request_base(&self, branch_state: &BranchState, old_target: &str) -> Result<()> {
        let new_target = &branch_state.target_branch;
        
        log::info!("🔄 Updating PR base: {} ({} → {})", 
                  branch_state.branch, old_target, new_target);
        
        match self.github_cli.update_pr_base(&branch_state.branch, new_target) {
            Ok(()) => {
                log::info!("✅ Updated PR base for {}", branch_state.branch);
                Ok(())
            }
            Err(YggitError::PullRequest(_)) => {
                log::info!("ℹ️  No existing PR found for {}. Creating new PR...", branch_state.branch);
                self.create_pull_request(branch_state, new_target)
            }
            Err(e) => {
                log::error!("❌ Failed to update PR for {}: {}", branch_state.branch, e);
                Err(e)
            }
        }
    }
}

/// Extract branch states from EnhancedCommits (with notes)
pub fn extract_branch_state(commits: &[EnhancedCommit<Note>]) -> HashMap<String, BranchState> {
    let mut states = HashMap::new();
    
    for commit in commits {
        if let Some(note) = &commit.note {
            if let Some(push) = &note.push {
                let target_branch = push.parent_branch.as_ref()
                    .unwrap_or(&"main".to_string())
                    .clone();
                
                let state = BranchState {
                    branch: push.branch.clone(),
                    target_branch,
                    origin: push.origin.clone(),
                    commit_title: Some(commit.title.clone()),
                    commit_description: commit.description.clone(),
                };
                
                states.insert(push.branch.clone(), state);
            }
        }
    }
    
    states
}

/// Extract branch states from parsed commits (before notes are saved)
pub fn extract_branch_state_from_parsed(commits: &[ParsedCommit]) -> HashMap<String, BranchState> {
    let mut states = HashMap::new();
    
    for commit in commits {
        if let Some(target) = &commit.target {
            let target_branch = target.parent_branch.as_ref()
                .unwrap_or(&"main".to_string())
                .clone();
            
            let state = BranchState {
                branch: target.branch.clone(),
                target_branch,
                origin: target.origin.clone(),
                commit_title: Some(commit.title.clone()),
                commit_description: None, // ParsedCommit doesn't have description
            };
            
            states.insert(target.branch.clone(), state);
        }
    }
    
    states
}