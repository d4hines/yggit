use crate::{
    core::{push_from_notes, save_note},
    errors::Result,
    git::Git,
    github::{GitHubCliImpl, GitHubIntegration, extract_branch_state, extract_branch_state_from_parsed},
    parser::commits_to_string,
};
use clap::Args;

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
    pub fn execute(&self, git: Git) -> Result<()> {
        // Step 1: Capture the current state (before editing)
        let before_commits = git.list_commits();
        let before_state = extract_branch_state(&before_commits);
        
        let output = commits_to_string(before_commits);

        let file_path = "/tmp/yggit";

        let output = format!("{}\n{}", output, COMMENTS);
        std::fs::write(file_path, output).map_err(|e| {
            log::error!("Cannot write file to disk: {}", e);
            e
        })?;

        let content = git.edit_file(file_path)?;

        // Get the actual main branch name (main or master)
        let main_branch_name = git.main_branch()
            .and_then(|branch| branch.name().ok().flatten().map(|s| s.to_string()))
            .unwrap_or_else(|| "main".to_string());
            
        let after_commits = crate::parser::instruction_from_string_with_main_branch(content, main_branch_name.clone())
            .ok_or_else(|| {
                log::error!("Cannot parse instructions");
                crate::errors::YggitError::Parse("Failed to parse instructions".to_string())
            })?;

        // Step 2: Extract the new state (after editing)
        let after_state = extract_branch_state_from_parsed(&after_commits);

        save_note(&git, after_commits);

        push_from_notes(&git);

        // Step 3: Handle GitHub PR integration (unless --no-pr flag is used)
        if !self.no_pr {
            let github_cli = GitHubCliImpl::new();
            let github_integration = GitHubIntegration::new(github_cli);
            github_integration.handle_integration(&before_state, &after_state, &main_branch_name)?;
        } else {
            log::info!("⏭️  Skipping GitHub PR integration (--no-pr flag used)");
        }

        Ok(())
    }
}

