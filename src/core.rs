use crate::{
    git::{EnhancedCommit, Git},
    parser::Target,
};
use serde::{Deserialize, Serialize};
use git2::Oid;

/// Trait for Git operations that can be mocked for testing
pub trait GitOperations {
    fn set_branch_to_commit_with_parent(&self, branch: &str, oid: Oid, parent_branch: Option<&str>) -> Result<(), ()>;
    fn head_of(&self, branch: &str) -> Option<Oid>;
}

impl GitOperations for Git {
    fn set_branch_to_commit_with_parent(&self, branch: &str, oid: Oid, parent_branch: Option<&str>) -> Result<(), ()> {
        Git::set_branch_to_commit_with_parent(self, branch, oid, parent_branch)
    }
    
    fn head_of(&self, branch: &str) -> Option<Oid> {
        Git::head_of(self, branch)
    }
}

#[derive(Deserialize, Serialize)]
pub struct Push {
    pub origin: Option<String>,
    pub branch: String,
    pub parent_branch: Option<String>,
}

#[derive(Deserialize, Serialize)]
pub struct Note {
    pub push: Option<Push>,
}

/// Process DAG branch creation with any GitOperations implementation (for testing)
pub fn process_dag_operations<T: GitOperations>(git_ops: &T, commits: &[EnhancedCommit<Note>]) -> Vec<(String, bool)> {
    let mut results = Vec::new();
    
    for commit in commits {
        let EnhancedCommit {
            id,
            note:
                Some(Note {
                    push: Some(Push { branch, origin: _, parent_branch }),
                    ..
                }),
            ..
        } = commit
        else {
            continue;
        };
        
        let success = git_ops.set_branch_to_commit_with_parent(branch, *id, parent_branch.as_deref()).is_ok();
        results.push((branch.clone(), success));
    }
    
    results
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
                push: target.map(|Target { origin, branch, parent_branch }| Push { origin, branch, parent_branch }),
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
                    push: Some(Push { branch, origin: _, parent_branch }),
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
                    push: Some(Push { origin, branch, parent_branch: _ }),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::cell::RefCell;
    use crate::parser::{instruction_from_string, Target};

    /// Mock Git implementation for testing DAG operations
    struct MockGit {
        branches: RefCell<HashMap<String, Oid>>,
        operations: RefCell<Vec<String>>,
    }

    impl MockGit {
        fn new() -> Self {
            let mut branches = HashMap::new();
            // Add main branch
            branches.insert("main".to_string(), Oid::from_str("1111111111111111111111111111111111111111").unwrap());
            
            Self {
                branches: RefCell::new(branches),
                operations: RefCell::new(Vec::new()),
            }
        }

        fn get_operations(&self) -> Vec<String> {
            self.operations.borrow().clone()
        }
    }

    impl GitOperations for MockGit {
        fn set_branch_to_commit_with_parent(&self, branch: &str, oid: Oid, parent_branch: Option<&str>) -> Result<(), ()> {
            let operation = match parent_branch {
                Some(parent) => format!("create {} -> {} (from {})", branch, oid, parent),
                None => format!("create {} -> {}", branch, oid),
            };
            
            self.operations.borrow_mut().push(operation);
            
            // Simulate setting the branch
            self.branches.borrow_mut().insert(branch.to_string(), oid);
            
            Ok(())
        }

        fn head_of(&self, branch: &str) -> Option<Oid> {
            self.branches.borrow().get(branch).copied()
        }
    }

    #[test]
    fn test_dag_operations_with_parent_branches() {
        let mock_git = MockGit::new();
        
        // Create test commits
        let commits = vec![
            EnhancedCommit {
                id: Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
                title: "First commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-1".to_string(),
                        parent_branch: None, // Will use main as default
                    }),
                }),
            },
            EnhancedCommit {
                id: Oid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
                title: "Second commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-2".to_string(),
                        parent_branch: Some("feature-1".to_string()),
                    }),
                }),
            },
            EnhancedCommit {
                id: Oid::from_str("cccccccccccccccccccccccccccccccccccccccc").unwrap(),
                title: "Third commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-3".to_string(),
                        parent_branch: Some("main".to_string()),
                    }),
                }),
            },
        ];

        let results = process_dag_operations(&mock_git, &commits);
        
        // Verify all operations succeeded
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, success)| *success));
        
        // Verify branch names
        let branch_names: Vec<_> = results.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(branch_names, vec!["feature-1", "feature-2", "feature-3"]);
        
        // Verify operations were called correctly
        let operations = mock_git.get_operations();
        assert_eq!(operations.len(), 3);
        
        // Check that parent relationships are tracked
        assert!(operations[1].contains("feature-1")); // feature-2 should reference feature-1
        assert!(operations[2].contains("main")); // feature-3 should reference main
    }

    #[test]
    fn test_implicit_parent_chain_operations() {
        let mock_git = MockGit::new();
        
        // Test the user's example: -> foo => bar, -> baz => bar, -> bam (bam should inherit from baz)
        let commits = vec![
            EnhancedCommit {
                id: Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
                title: "First commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "foo".to_string(),
                        parent_branch: Some("bar".to_string()),
                    }),
                }),
            },
            EnhancedCommit {
                id: Oid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
                title: "Second commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "baz".to_string(),
                        parent_branch: Some("bar".to_string()),
                    }),
                }),
            },
            EnhancedCommit {
                id: Oid::from_str("cccccccccccccccccccccccccccccccccccccccc").unwrap(),
                title: "Third commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "bam".to_string(),
                        parent_branch: Some("baz".to_string()), // This should be set by the parser
                    }),
                }),
            },
        ];

        let results = process_dag_operations(&mock_git, &commits);
        
        // Verify all operations succeeded
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, success)| *success));
        
        // Verify the operations contain the expected parent relationships
        let operations = mock_git.get_operations();
        assert_eq!(operations.len(), 3);
        
        // foo and baz should both reference bar
        assert!(operations[0].contains("(from bar)"));
        assert!(operations[1].contains("(from bar)"));
        
        // bam should reference baz (the implicit parent from the parser)
        assert!(operations[2].contains("(from baz)"));
    }

    #[test]
    fn test_end_to_end_dag_workflow() {
        
        // Test the complete workflow: Parse → Save Notes → Process DAG operations
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e First commit\n-> foo => bar\n\n9d25845c91ff1aac84dbffd96664d8d6c16dccb2 Second commit\n-> baz => bar\n\nae36956d02aa2bce95ecbba07775e9e7d27edde3 Third commit\n-> bam\n";
        
        // Step 1: Parse the input (this should set bam's parent to baz)
        let parsed_commits = instruction_from_string(input.to_string()).expect("Should parse");
        assert_eq!(parsed_commits.len(), 3);
        
        // Verify parsing worked correctly (including implicit parent)
        assert_eq!(parsed_commits[0].target.as_ref().unwrap().branch, "foo");
        assert_eq!(parsed_commits[0].target.as_ref().unwrap().parent_branch, Some("bar".to_string()));
        
        assert_eq!(parsed_commits[1].target.as_ref().unwrap().branch, "baz");
        assert_eq!(parsed_commits[1].target.as_ref().unwrap().parent_branch, Some("bar".to_string()));
        
        assert_eq!(parsed_commits[2].target.as_ref().unwrap().branch, "bam");
        assert_eq!(parsed_commits[2].target.as_ref().unwrap().parent_branch, Some("baz".to_string())); // Implicit parent
        
        // Step 2: Convert to EnhancedCommits with Notes (simulating save_note behavior)
        let enhanced_commits: Vec<EnhancedCommit<Note>> = parsed_commits.iter().map(|commit| {
            let note = Some(Note {
                push: commit.target.as_ref().map(|Target { origin, branch, parent_branch }| Push {
                    origin: origin.clone(),
                    branch: branch.clone(),
                    parent_branch: parent_branch.clone(),
                }),
            });
            
            EnhancedCommit {
                id: commit.hash,
                title: commit.title.clone(),
                description: None,
                note,
            }
        }).collect();
        
        // Step 3: Process DAG operations
        let mock_git = MockGit::new();
        let results = process_dag_operations(&mock_git, &enhanced_commits);
        
        // Step 4: Verify all operations succeeded
        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|(_, success)| *success));
        
        // Step 5: Verify the DAG structure was created correctly
        let operations = mock_git.get_operations();
        assert_eq!(operations.len(), 3);
        
        // Verify foo and baz both reference bar
        assert!(operations[0].contains("foo") && operations[0].contains("(from bar)"));
        assert!(operations[1].contains("baz") && operations[1].contains("(from bar)"));
        
        // Verify bam references baz (the implicit parent)
        assert!(operations[2].contains("bam") && operations[2].contains("(from baz)"));
    }

    #[test]
    fn test_complex_dag_scenarios() {
        // Test more complex DAG structures
        let mock_git = MockGit::new();
        
        let commits = vec![
            // Linear chain
            EnhancedCommit {
                id: Oid::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap(),
                title: "Feature A".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-a".to_string(),
                        parent_branch: None, // Will default to main
                    }),
                }),
            },
            // Branch from feature-a
            EnhancedCommit {
                id: Oid::from_str("bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb").unwrap(),
                title: "Feature B".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-b".to_string(),
                        parent_branch: Some("feature-a".to_string()),
                    }),
                }),
            },
            // Another branch from feature-a (parallel to feature-b)
            EnhancedCommit {
                id: Oid::from_str("cccccccccccccccccccccccccccccccccccccccc").unwrap(),
                title: "Feature C".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-c".to_string(),
                        parent_branch: Some("feature-a".to_string()),
                    }),
                }),
            },
            // Branch directly from main (independent)
            EnhancedCommit {
                id: Oid::from_str("dddddddddddddddddddddddddddddddddddddddd").unwrap(),
                title: "Feature D".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-d".to_string(),
                        parent_branch: Some("main".to_string()),
                    }),
                }),
            },
        ];

        let results = process_dag_operations(&mock_git, &commits);
        
        // All operations should succeed
        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|(_, success)| *success));
        
        let operations = mock_git.get_operations();
        assert_eq!(operations.len(), 4);
        
        // Verify the DAG structure:
        // feature-a -> (main)
        // feature-b -> feature-a  
        // feature-c -> feature-a
        // feature-d -> main
        
        // feature-a should have no explicit parent (will default to main in implementation)
        assert!(operations[0].contains("feature-a"));
        
        // feature-b and feature-c should both reference feature-a
        assert!(operations[1].contains("feature-b") && operations[1].contains("(from feature-a)"));
        assert!(operations[2].contains("feature-c") && operations[2].contains("(from feature-a)"));
        
        // feature-d should reference main
        assert!(operations[3].contains("feature-d") && operations[3].contains("(from main)"));
    }

    #[test]
    fn test_mixed_explicit_and_implicit_parents() {
        // Test a scenario with both explicit parents (=>) and implicit parents
        let input = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa First\n-> alpha\n\nbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb Second\n-> beta => main\n\ncccccccccccccccccccccccccccccccccccccccc Third\n-> gamma\n\ndddddddddddddddddddddddddddddddddddddddd Fourth\n-> delta => alpha\n";
        
        let parsed_commits = instruction_from_string(input.to_string()).expect("Should parse");
        assert_eq!(parsed_commits.len(), 4);
        
        // Verify parent relationships:
        // alpha: implicit parent = main (first)
        // beta: explicit parent = main  
        // gamma: implicit parent = beta (from previous)
        // delta: explicit parent = alpha
        
        assert_eq!(parsed_commits[0].target.as_ref().unwrap().parent_branch, Some("main".to_string()));
        assert_eq!(parsed_commits[1].target.as_ref().unwrap().parent_branch, Some("main".to_string()));
        assert_eq!(parsed_commits[2].target.as_ref().unwrap().parent_branch, Some("beta".to_string())); // implicit
        assert_eq!(parsed_commits[3].target.as_ref().unwrap().parent_branch, Some("alpha".to_string())); // explicit
    }
}
