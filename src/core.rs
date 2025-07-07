use crate::{
    git::{EnhancedCommit, Git},
    parser::Target,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Push {
    pub origin: Option<String>,
    pub branch: String,
    pub parent_branch: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Note {
    pub push: Option<Push>,
}

/// Save the note to the commit
///
/// Also deletes note if there is nothing new
pub fn save_note(git: &Git, commits: Vec<crate::parser::Commit>) {
    for commit in commits {
        // Extract information from commit
        let crate::parser::Commit { hash, target, .. } = commit;

        let is_empty = target.is_none();

        if is_empty {
            git.delete_note(&hash);
        } else {
            // Create the note
            let note = Note {
                push: target.map(
                    |Target {
                         origin,
                         branch,
                         parent_branch,
                     }| Push {
                        origin,
                        branch,
                        parent_branch,
                    },
                ),
            };

            // Save the note
            git.set_note(hash, note).unwrap();
        }
    }
}

/// Execute the push instructions from the notes
///
/// Change the head of the given branches with proper DAG relationships
/// Push the branches to origin
pub fn push_from_notes(git: &Git) {
    let commits = git.list_commits();

    // Process commits in order to handle parent dependencies
    // The commits are already in the correct order from the git log
    for commit in &commits {
        let EnhancedCommit {
            id,
            note:
                Some(Note {
                    push:
                        Some(Push {
                            branch,
                            origin: _,
                            parent_branch,
                        }),
                    ..
                }),
            ..
        } = commit
        else {
            continue;
        };

        // Set the head of the branch to the given commit, creating proper DAG relationships
        match git.set_branch_to_commit_with_parent(branch, *id, parent_branch.as_deref()) {
            Ok(()) => {
                if let Some(parent) = parent_branch {
                    println!("✅ Created branch '{}' from parent '{}'", branch, parent);
                } else {
                    println!("✅ Created branch '{}'", branch);
                }
            }
            Err(()) => {
                eprintln!("❌ Failed to create branch '{}'", branch);
                if let Some(parent) = parent_branch {
                    eprintln!("   Parent branch '{}' may not exist", parent);
                }
            }
        }
    }

    // Push everything
    for commit in &commits {
        let EnhancedCommit {
            note:
                Some(Note {
                    push:
                        Some(Push {
                            origin,
                            branch,
                            parent_branch: _,
                        }),
                    ..
                }),
            ..
        } = commit
        else {
            continue;
        };

        let origin = origin
            .clone()
            .unwrap_or(git.config.yggit.default_upstream.clone());

        let local_remote_commit = git.find_local_remote_head(&origin, branch);
        let remote_commit = git.find_remote_head(&origin, branch);
        let local_commit = git.head_of(branch);

        if local_remote_commit != remote_commit {
            println!("cannot push {}", branch);
            return;
        }

        if local_commit == remote_commit {
            println!("{}:{} is up to date", origin, branch);
            continue;
        }

        println!("pushing {}:{}", origin, branch);
        git.push_force(&origin, branch);
        println!("\r{}:{} pushed", origin, branch);
    }
}
