use crate::{
    core::{apply_notes, process_instructions, push_branches},
    git::Git,
    parser::{commits_to_string, instruction_from_string},
};
use clap::Args;

use super::Execute;

#[derive(Debug, Args)]
pub struct Push {}

const COMMENTS: &str = r#"
# Here is how to use yggit
# 
# Commands:
# -> <branch> add a branch to the above commit
# 
# What happens next?
#  - All branches are pushed
#
# It's not a rebase, you can't edit commits nor reorder them
"#;

impl Execute for Push {
    fn execute(&self) -> Result<(), ()> {
        let git = Git::open(".");

        let commits = git.list_commits();
        let output = commits_to_string(commits);

        let file_path = "/tmp/yggit";

        let output = format!("{}\n{}", output, COMMENTS);
        std::fs::write(file_path, output).map_err(|_| println!("cannot write file to disk"))?;

        let content = git.edit_file(file_path)?;

        let instructions = instruction_from_string(content).ok_or_else(|| {
            println!("Cannot parse instructions");
        })?;

        process_instructions(&git, instructions);

        // updates branches
        apply_notes(&git);

        // push
        push_branches(&git);

        Ok(())
    }
}
