// Git related

use crate::{
    core::{Note, Push},
    git::EnhancedCommit,
};
use git2::Oid;
use regex::Regex;

pub fn commits_to_string(commits: Vec<EnhancedCommit<Note>>) -> String {
    let mut output = String::default();
    for commit in commits {
        output = format!("{}{} {}\n", output, commit.id, commit.title);
        if let Some(Note { push }) = commit.note {
            if let Some(Push {
                origin: Some(origin),
                branch,
                parent_branch,
            }) = &push
            {
                if let Some(parent) = parent_branch {
                    output = format!("{}-> {}:{} => {}\n", output, origin, branch, parent);
                } else {
                    output = format!("{}-> {}:{}\n", output, origin, branch);
                }
            } else if let Some(Push {
                origin: None,
                branch,
                parent_branch,
            }) = &push
            {
                if let Some(parent) = parent_branch {
                    output = format!("{}-> {} => {}\n", output, branch, parent);
                } else {
                    output = format!("{}-> {}\n", output, branch);
                }
            }
            // An empty line is added so that it is cleaner to differentiate the different MR
            if push.is_some() {
                output = format!("{}\n", output);
            }
        }
    }
    output
}

#[derive(Debug, Clone)]
pub struct Target {
    pub origin: Option<String>,
    pub branch: String,
    pub parent_branch: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: Oid,
    #[allow(dead_code)]
    pub title: String,
    pub target: Option<Target>,
}

pub fn instruction_from_string(input: String) -> Option<Vec<Commit>> {
    instruction_from_string_with_main_branch(input, "main".to_string())
}

pub fn instruction_from_string_with_main_branch(input: String, main_branch_name: String) -> Option<Vec<Commit>> {
    let commit_header_re = Regex::new(r"^(?P<hash>[0-9a-fA-F]{40})\s+(?P<title>.+)$").ok()?;
    let target_re = Regex::new(r"^->\s*(?:(?P<origin>[^:]+):)?(?P<branch>[^=]+?)(?:\s*=>\s*(?P<parent>.+))?$").ok()?;
    
    let mut commits = Vec::new();
    let lines: Vec<&str> = input.lines().map(|line| line.trim()).collect();
    let mut i = 0;
    let mut last_branch: Option<String> = None;
    while i < lines.len() {
        let line = lines[i];
        if line.is_empty() || line.starts_with("#") {
            i += 1;
            continue;
        }
        if let Some(caps) = commit_header_re.captures(line) {
            if let (Some(hash_str), Some(title_str)) = (caps.name("hash"), caps.name("title")) {
                if let Ok(hash) = Oid::from_str(hash_str.as_str()) {
                    let title = title_str.as_str().to_string();
                    let mut target = None;
                    if i + 1 < lines.len() {
                        let next_line = lines[i + 1];
                        if next_line.starts_with("->") {
                            if let Some(target_caps) = target_re.captures(next_line) {
                                if let Some(branch_cap) = target_caps.name("branch") {
                                    let origin = target_caps.name("origin").map(|m| m.as_str().to_string());
                                    let branch = branch_cap.as_str().trim().to_string();
                                    let mut parent_branch = target_caps.name("parent").map(|m| m.as_str().trim().to_string());
                                    
                                    // If no explicit parent specified, use the last branch or main branch if first
                                    if parent_branch.is_none() {
                                        parent_branch = last_branch.clone().or_else(|| Some(main_branch_name.clone()));
                                    }
                                    
                                    target = Some(Target { origin, branch: branch.clone(), parent_branch });
                                    last_branch = Some(branch);
                                    i += 1;
                                }
                            }
                        }
                    }
                    commits.push(Commit { hash, title, target });
                }
            }
        }
        i += 1;
    }
    if commits.is_empty() {
        None
    } else {
        Some(commits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_commit_with_target_no_colon() {
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e devinfra: New configs (#3333)\n-> d4hines/foo-bar\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 1);
        let commit = &commits[0];
        let expected_hash = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e";
        assert_eq!(commit.hash.to_string(), expected_hash);
        assert_eq!(commit.title, "devinfra: New configs (#3333)".to_string());
        assert!(commit.target.is_some());
        let target = commit.target.as_ref().unwrap();
        assert_eq!(target.origin, None);
        assert_eq!(target.branch, "d4hines/foo-bar".to_string());
        assert_eq!(target.parent_branch, Some("main".to_string()));
    }

    #[test]
    fn test_parse_commit_without_target() {
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e Some commit without target\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 1);
        let commit = &commits[0];
        let expected_hash = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e";
        assert_eq!(commit.hash.to_string(), expected_hash);
        assert_eq!(commit.title, "Some commit without target".to_string());
        assert!(commit.target.is_none());
    }

    #[test]
    fn test_parse_commit_with_target_with_colon() {
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e Feature commit with colon\n-> d4hines:foo-bar\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 1);
        let commit = &commits[0];
        let expected_hash = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e";
        assert_eq!(commit.hash.to_string(), expected_hash);
        assert_eq!(commit.title, "Feature commit with colon".to_string());
        assert!(commit.target.is_some());
        let target = commit.target.as_ref().unwrap();
        assert_eq!(target.origin, Some("d4hines".to_string()));
        assert_eq!(target.branch, "foo-bar".to_string());
        assert_eq!(target.parent_branch, Some("main".to_string()));
    }

    #[test]
    fn test_parse_commit_with_parent_branch() {
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e Feature with parent\n-> feature-branch => main\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 1);
        let commit = &commits[0];
        let expected_hash = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e";
        assert_eq!(commit.hash.to_string(), expected_hash);
        assert_eq!(commit.title, "Feature with parent".to_string());
        assert!(commit.target.is_some());
        let target = commit.target.as_ref().unwrap();
        assert_eq!(target.origin, None);
        assert_eq!(target.branch, "feature-branch".to_string());
        assert_eq!(target.parent_branch, Some("main".to_string()));
    }

    #[test]
    fn test_parse_commit_with_origin_and_parent_branch() {
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e Feature with origin and parent\n-> origin:feature-branch => develop\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 1);
        let commit = &commits[0];
        let expected_hash = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e";
        assert_eq!(commit.hash.to_string(), expected_hash);
        assert_eq!(commit.title, "Feature with origin and parent".to_string());
        assert!(commit.target.is_some());
        let target = commit.target.as_ref().unwrap();
        assert_eq!(target.origin, Some("origin".to_string()));
        assert_eq!(target.branch, "feature-branch".to_string());
        assert_eq!(target.parent_branch, Some("develop".to_string()));
    }

    #[test]
    fn test_parse_multiple_commits_with_dag_structure() {
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e First commit\n-> feature-1\n\n9d25845c91ff1aac84dbffd96664d8d6c16dccb2 Second commit\n-> feature-2 => feature-1\n\nae36956d02aa2bce95ecbba07775e9e7d27edde3 Third commit\n-> feature-3 => main\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 3);
        
        // First commit (linear from main)
        let commit1 = &commits[0];
        assert_eq!(commit1.title, "First commit".to_string());
        let target1 = commit1.target.as_ref().unwrap();
        assert_eq!(target1.branch, "feature-1".to_string());
        assert_eq!(target1.parent_branch, Some("main".to_string()));
        
        // Second commit (branches from feature-1)
        let commit2 = &commits[1];
        assert_eq!(commit2.title, "Second commit".to_string());
        let target2 = commit2.target.as_ref().unwrap();
        assert_eq!(target2.branch, "feature-2".to_string());
        assert_eq!(target2.parent_branch, Some("feature-1".to_string()));
        
        // Third commit (branches from main, creating DAG)
        let commit3 = &commits[2];
        assert_eq!(commit3.title, "Third commit".to_string());
        let target3 = commit3.target.as_ref().unwrap();
        assert_eq!(target3.branch, "feature-3".to_string());
        assert_eq!(target3.parent_branch, Some("main".to_string()));
    }

    #[test]
    fn test_example_from_user() {
        // Test the exact scenario mentioned: -> foo => bar, -> baz => bar, -> bam
        // bam should have parent of baz (the last branch)
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e First commit\n-> foo => bar\n\n9d25845c91ff1aac84dbffd96664d8d6c16dccb2 Second commit\n-> baz => bar\n\nae36956d02aa2bce95ecbba07775e9e7d27edde3 Third commit\n-> bam\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 3);

        // First commit: foo => bar (explicit parent)
        let target1 = commits[0].target.as_ref().unwrap();
        assert_eq!(target1.branch, "foo".to_string());
        assert_eq!(target1.parent_branch, Some("bar".to_string()));

        // Second commit: baz => bar (explicit parent)  
        let target2 = commits[1].target.as_ref().unwrap();
        assert_eq!(target2.branch, "baz".to_string());
        assert_eq!(target2.parent_branch, Some("bar".to_string()));

        // Third commit: bam (implicit parent should be "baz" from previous commit)
        let target3 = commits[2].target.as_ref().unwrap();
        assert_eq!(target3.branch, "bam".to_string());
        assert_eq!(target3.parent_branch, Some("baz".to_string()));
    }

    #[test]
    fn test_linear_chain_with_implicit_parents() {
        let input = "8c14734b80ff0ffb93caefc85553c7c5b05cca1e First commit\n-> feature-1\n\n9d25845c91ff1aac84dbffd96664d8d6c16dccb2 Second commit\n-> feature-2\n\nae36956d02aa2bce95ecbba07775e9e7d27edde3 Third commit\n-> feature-3\n";
        let commits = instruction_from_string(input.to_string()).expect("Should parse commits");
        assert_eq!(commits.len(), 3);

        // First commit: implicit parent should be "main" (first in chain)
        let target1 = commits[0].target.as_ref().unwrap();
        assert_eq!(target1.branch, "feature-1".to_string());
        assert_eq!(target1.parent_branch, Some("main".to_string()));

        // Second commit: implicit parent should be "feature-1"
        let target2 = commits[1].target.as_ref().unwrap();
        assert_eq!(target2.branch, "feature-2".to_string());
        assert_eq!(target2.parent_branch, Some("feature-1".to_string()));

        // Third commit: implicit parent should be "feature-2"
        let target3 = commits[2].target.as_ref().unwrap();
        assert_eq!(target3.branch, "feature-3".to_string());
        assert_eq!(target3.parent_branch, Some("feature-2".to_string()));
    }

    #[test]
    fn test_commits_to_string_shows_implicit_parents() {
        use crate::core::{Note, Push};
        use crate::git::EnhancedCommit;
        
        // Create commits with implicit parent relationships
        let commits = vec![
            EnhancedCommit {
                id: Oid::from_str("8c14734b80ff0ffb93caefc85553c7c5b05cca1e").unwrap(),
                title: "First commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-1".to_string(),
                        parent_branch: Some("main".to_string()), // Default to main
                    }),
                }),
            },
            EnhancedCommit {
                id: Oid::from_str("9d25845c91ff1aac84dbffd96664d8d6c16dccb2").unwrap(),
                title: "Second commit".to_string(),
                description: None,
                note: Some(Note {
                    push: Some(Push {
                        origin: None,
                        branch: "feature-2".to_string(),
                        parent_branch: Some("feature-1".to_string()), // Implicit parent
                    }),
                }),
            },
        ];

        let output = commits_to_string(commits);
        
        // Verify that both explicit and implicit parents are shown
        assert!(output.contains("-> feature-1 => main\n")); // Default parent shown
        assert!(output.contains("-> feature-2 => feature-1\n")); // Implicit parent shown
    }
}
