use super::*;
use crate::{
    core::Note,
    git::EnhancedCommit,
    github::cli::MockGitHubCli,
    parser::Commit as ParsedCommit,
};
use git2::Oid;
use std::collections::HashMap;

fn create_test_oid(suffix: &str) -> Oid {
    Oid::from_str(&format!("{}aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", suffix)).unwrap()
}

fn create_enhanced_commit(id: &str, title: &str, branch: &str, target: Option<&str>) -> EnhancedCommit<Note> {
    let push = Some(crate::core::Push {
        origin: None,
        branch: branch.to_string(),
        parent_branch: target.map(|t| t.to_string()),
    });
    
    EnhancedCommit {
        id: create_test_oid(id),
        title: title.to_string(),
        description: Some(format!("Description for {}", title)),
        note: Some(Note { push }),
    }
}

fn create_parsed_commit(id: &str, title: &str, branch: &str, target: Option<&str>) -> ParsedCommit {
    let target = Some(crate::parser::Target {
        origin: None,
        branch: branch.to_string(),
        parent_branch: target.map(|t| t.to_string()),
    });
    
    ParsedCommit {
        hash: create_test_oid(id),
        title: title.to_string(),
        target,
    }
}

#[test]
fn test_extract_branch_state_from_enhanced_commits() {
    let commits = vec![
        create_enhanced_commit("a", "First commit", "feature-1", None),
        create_enhanced_commit("b", "Second commit", "feature-2", Some("main")),
    ];
    
    let states = extract_branch_state(&commits);
    
    assert_eq!(states.len(), 2);
    
    let feature1 = &states["feature-1"];
    assert_eq!(feature1.branch, "feature-1");
    assert_eq!(feature1.target_branch, "main"); // Default when parent_branch is None
    assert_eq!(feature1.commit_title, Some("First commit".to_string()));
    assert_eq!(feature1.commit_description, Some("Description for First commit".to_string()));
    
    let feature2 = &states["feature-2"];
    assert_eq!(feature2.branch, "feature-2");
    assert_eq!(feature2.target_branch, "main");
    assert_eq!(feature2.commit_title, Some("Second commit".to_string()));
}

#[test]
fn test_extract_branch_state_from_parsed_commits() {
    let commits = vec![
        create_parsed_commit("a", "First commit", "feature-1", None),
        create_parsed_commit("b", "Second commit", "feature-2", Some("feature-1")),
    ];
    
    let states = extract_branch_state_from_parsed(&commits);
    
    assert_eq!(states.len(), 2);
    
    let feature1 = &states["feature-1"];
    assert_eq!(feature1.branch, "feature-1");
    assert_eq!(feature1.target_branch, "main");
    assert_eq!(feature1.commit_description, None); // ParsedCommit doesn't have description
    
    let feature2 = &states["feature-2"];
    assert_eq!(feature2.branch, "feature-2");
    assert_eq!(feature2.target_branch, "feature-1");
}

#[test]
fn test_github_integration_new_branch() {
    let github_cli = MockGitHubCli::new();
    let integration = GitHubIntegration::new(github_cli);
    
    let before_state = HashMap::new();
    let mut after_state = HashMap::new();
    after_state.insert("feature-1".to_string(), BranchState {
        branch: "feature-1".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("New feature".to_string()),
        commit_description: Some("Feature description".to_string()),
    });
    
    let result = integration.handle_integration(&before_state, &after_state, "main");
    assert!(result.is_ok());
    
    let created_prs = integration.github_cli.get_created_prs();
    assert_eq!(created_prs.len(), 1);
    assert_eq!(created_prs[0].0, "feature-1");
    assert_eq!(created_prs[0].1, "main");
    assert_eq!(created_prs[0].2, "New feature");
}

#[test]
fn test_github_integration_target_change() {
    let github_cli = MockGitHubCli::new().with_existing_prs(vec!["feature-1".to_string()]);
    let integration = GitHubIntegration::new(github_cli);
    
    let mut before_state = HashMap::new();
    before_state.insert("feature-1".to_string(), BranchState {
        branch: "feature-1".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Feature".to_string()),
        commit_description: Some("Description".to_string()),
    });
    
    let mut after_state = HashMap::new();
    after_state.insert("feature-1".to_string(), BranchState {
        branch: "feature-1".to_string(),
        target_branch: "develop".to_string(), // Changed target
        origin: None,
        commit_title: Some("Feature".to_string()),
        commit_description: Some("Description".to_string()),
    });
    
    let result = integration.handle_integration(&before_state, &after_state, "main");
    assert!(result.is_ok());
    
    let updated_prs = integration.github_cli.get_updated_prs();
    assert_eq!(updated_prs.len(), 1);
    assert_eq!(updated_prs[0].0, "feature-1");
    assert_eq!(updated_prs[0].1, "develop");
}

#[test]
fn test_github_integration_missing_pr_for_existing_branch() {
    let github_cli = MockGitHubCli::new(); // No existing PRs
    let integration = GitHubIntegration::new(github_cli);
    
    let mut before_state = HashMap::new();
    before_state.insert("feature-1".to_string(), BranchState {
        branch: "feature-1".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Feature".to_string()),
        commit_description: Some("Description".to_string()),
    });
    
    let after_state = before_state.clone();
    
    let result = integration.handle_integration(&before_state, &after_state, "main");
    assert!(result.is_ok());
    
    // Should create PR for existing branch without PR
    let created_prs = integration.github_cli.get_created_prs();
    assert_eq!(created_prs.len(), 1);
    assert_eq!(created_prs[0].0, "feature-1");
}

#[test]
fn test_github_integration_cli_not_available() {
    let github_cli = MockGitHubCli::new().set_available(false);
    let integration = GitHubIntegration::new(github_cli);
    
    let before_state = HashMap::new();
    let mut after_state = HashMap::new();
    after_state.insert("feature-1".to_string(), BranchState {
        branch: "feature-1".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Feature".to_string()),
        commit_description: None,
    });
    
    let result = integration.handle_integration(&before_state, &after_state, "main");
    assert!(result.is_ok());
    
    // No PRs should be created when CLI is not available
    let created_prs = integration.github_cli.get_created_prs();
    assert_eq!(created_prs.len(), 0);
}

#[test]
fn test_find_branch_with_description() {
    let github_cli = MockGitHubCli::new();
    let integration = GitHubIntegration::new(github_cli);
    
    let after_branch = BranchState {
        branch: "new-feature".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Add feature".to_string()),
        commit_description: None,
    };
    
    let mut before_state = HashMap::new();
    before_state.insert("old-feature".to_string(), BranchState {
        branch: "old-feature".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Add feature".to_string()), // Same title
        commit_description: Some("Detailed description".to_string()),
    });
    
    let result = integration.find_branch_with_description(&after_branch, &before_state);
    
    assert_eq!(result.branch, "new-feature");
    assert_eq!(result.commit_description, Some("Detailed description".to_string()));
}

#[test]
fn test_complex_workflow_scenario() {
    let github_cli = MockGitHubCli::new()
        .with_existing_prs(vec!["feature-1".to_string(), "feature-2".to_string()]);
    let integration = GitHubIntegration::new(github_cli);
    
    // Before state: two existing branches
    let mut before_state = HashMap::new();
    before_state.insert("feature-1".to_string(), BranchState {
        branch: "feature-1".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Feature 1".to_string()),
        commit_description: Some("Description 1".to_string()),
    });
    before_state.insert("feature-2".to_string(), BranchState {
        branch: "feature-2".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Feature 2".to_string()),
        commit_description: Some("Description 2".to_string()),
    });
    
    // After state: feature-1 changes target, feature-2 removed, feature-3 added
    let mut after_state = HashMap::new();
    after_state.insert("feature-1".to_string(), BranchState {
        branch: "feature-1".to_string(),
        target_branch: "develop".to_string(), // Changed target
        origin: None,
        commit_title: Some("Feature 1".to_string()),
        commit_description: Some("Description 1".to_string()),
    });
    after_state.insert("feature-3".to_string(), BranchState {
        branch: "feature-3".to_string(),
        target_branch: "main".to_string(),
        origin: None,
        commit_title: Some("Feature 3".to_string()),
        commit_description: Some("Description 3".to_string()),
    });
    
    let result = integration.handle_integration(&before_state, &after_state, "main");
    assert!(result.is_ok());
    
    // Check that feature-1 was updated
    let updated_prs = integration.github_cli.get_updated_prs();
    assert_eq!(updated_prs.len(), 1);
    assert_eq!(updated_prs[0].0, "feature-1");
    assert_eq!(updated_prs[0].1, "develop");
    
    // Check that feature-3 was created
    let created_prs = integration.github_cli.get_created_prs();
    assert_eq!(created_prs.len(), 1);
    assert_eq!(created_prs[0].0, "feature-3");
}

#[test]
fn test_branch_state_with_custom_origin() {
    let commits = vec![
        EnhancedCommit {
            id: create_test_oid("a"),
            title: "First commit".to_string(),
            description: Some("Description".to_string()),
            note: Some(Note {
                push: Some(crate::core::Push {
                    origin: Some("upstream".to_string()),
                    branch: "feature-1".to_string(),
                    parent_branch: Some("develop".to_string()),
                }),
            }),
        }
    ];
    
    let states = extract_branch_state(&commits);
    let feature1 = &states["feature-1"];
    
    assert_eq!(feature1.origin, Some("upstream".to_string()));
    assert_eq!(feature1.target_branch, "develop");
}