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
            }) = &push
            {
                output = format!("{}-> {}:{}\n", output, origin, branch);
            } else if let Some(Push {
                origin: None,
                branch,
            }) = &push
            {
                output = format!("{}-> {}\n", output, branch);
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
}

#[derive(Debug, Clone)]
pub struct Commit {
    pub hash: Oid,
    #[allow(dead_code)]
    pub title: String,
    pub target: Option<Target>,
}

pub fn instruction_from_string(input: String) -> Option<Vec<Commit>> {
    let commit_header_re = Regex::new(r"^(?P<hash>[0-9a-fA-F]{40})\s+(?P<title>.+)$").ok()?;
    let target_re = Regex::new(r"^->\s*(?:(?P<origin>[^:]+):)?(?P<branch>.+)$").ok()?;
    
    let mut commits = Vec::new();
    let lines: Vec<&str> = input.lines().map(|line| line.trim()).collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.is_empty() || line.starts_with("#") {
            i += 1;
            continue;
        }
        if let Some(caps) = commit_header_re.captures(line) {
            let hash_str = caps.name("hash")?.as_str();
            let title = caps.name("title")?.as_str().to_string();
            let hash = Oid::from_str(hash_str).ok()?;
            let mut target = None;
            if i + 1 < lines.len() {
                let next_line = lines[i + 1];
                if next_line.starts_with("->") {
                    if let Some(target_caps) = target_re.captures(next_line) {
                        let origin = target_caps.name("origin").map(|m| m.as_str().to_string());
                        let branch = target_caps.name("branch")?.as_str().to_string();
                        target = Some(Target { origin, branch });
                        i += 1;
                    }
                }
            }
            commits.push(Commit { hash, title, target });
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
    use git2::Oid;

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
    }
}
