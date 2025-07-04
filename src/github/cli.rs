use crate::errors::{Result, YggitError};
use std::collections::HashMap;
use std::process::Command;

pub trait GitHubCli {
    fn is_available(&self) -> Result<bool>;
    fn pr_exists(&self, branch_name: &str) -> Result<bool>;
    fn create_pr(&self, branch: &str, target: &str, title: &str, body: &str) -> Result<String>;
    fn update_pr_base(&self, branch: &str, new_base: &str) -> Result<()>;
}

pub struct GitHubCliImpl;

impl GitHubCliImpl {
    pub fn new() -> Self {
        Self
    }
    
    fn run_command(&self, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("gh")
            .args(args)
            .output()
            .map_err(|e| YggitError::GitHubCli(format!("Failed to execute gh command: {}", e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(YggitError::GitHubCli(stderr.to_string()));
        }
        
        Ok(output)
    }
}

impl GitHubCli for GitHubCliImpl {
    fn is_available(&self) -> Result<bool> {
        match Command::new("gh").arg("--version").output() {
            Ok(output) => Ok(output.status.success()),
            Err(_) => Ok(false),
        }
    }
    
    fn pr_exists(&self, branch_name: &str) -> Result<bool> {
        log::debug!("Checking if PR exists for branch: {}", branch_name);
        
        let output = self.run_command(&["pr", "list", "--head", branch_name, "--json", "number"])?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        
        // If the JSON output is "[]", no PRs exist for this branch
        let exists = !stdout.trim().eq("[]");
        log::debug!("PR exists for {}: {}", branch_name, exists);
        
        Ok(exists)
    }
    
    fn create_pr(&self, branch: &str, target: &str, title: &str, body: &str) -> Result<String> {
        log::info!("Creating PR: {} → {} (\"{}\")", branch, target, title);
        
        let output = self.run_command(&[
            "pr", "create",
            "--head", branch,
            "--base", target,
            "--title", title,
            "--body", body,
        ])?;
        
        let stdout = String::from_utf8_lossy(&output.stdout);
        let result = if !stdout.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            format!("Created PR for {}", branch)
        };
        
        log::info!("✅ {}", result);
        Ok(result)
    }
    
    fn update_pr_base(&self, branch: &str, new_base: &str) -> Result<()> {
        log::info!("Updating PR base for {}: → {}", branch, new_base);
        
        let output = self.run_command(&[
            "pr", "edit",
            branch,
            "--base", new_base,
        ]);
        
        match output {
            Ok(_) => {
                log::info!("✅ Updated PR base for {}", branch);
                Ok(())
            }
            Err(YggitError::GitHubCli(ref error)) if error.contains("not found") => {
                log::info!("ℹ️  No existing PR found for {}. Will create new PR.", branch);
                Err(YggitError::PullRequest(format!("PR not found for branch {}", branch)))
            }
            Err(e) => Err(e),
        }
    }
}

pub struct MockGitHubCli {
    pub available: bool,
    pub existing_prs: HashMap<String, bool>,
    pub created_prs: std::sync::Mutex<Vec<(String, String, String, String)>>,
    pub updated_prs: std::sync::Mutex<Vec<(String, String)>>,
}

impl MockGitHubCli {
    pub fn new() -> Self {
        Self {
            available: true,
            existing_prs: HashMap::new(),
            created_prs: std::sync::Mutex::new(Vec::new()),
            updated_prs: std::sync::Mutex::new(Vec::new()),
        }
    }
    
    pub fn with_existing_prs(mut self, prs: Vec<String>) -> Self {
        for pr in prs {
            self.existing_prs.insert(pr, true);
        }
        self
    }
    
    pub fn set_available(mut self, available: bool) -> Self {
        self.available = available;
        self
    }
    
    pub fn get_created_prs(&self) -> Vec<(String, String, String, String)> {
        self.created_prs.lock().unwrap().clone()
    }
    
    pub fn get_updated_prs(&self) -> Vec<(String, String)> {
        self.updated_prs.lock().unwrap().clone()
    }
}

impl GitHubCli for MockGitHubCli {
    fn is_available(&self) -> Result<bool> {
        Ok(self.available)
    }
    
    fn pr_exists(&self, branch_name: &str) -> Result<bool> {
        Ok(self.existing_prs.get(branch_name).copied().unwrap_or(false))
    }
    
    fn create_pr(&self, branch: &str, target: &str, title: &str, body: &str) -> Result<String> {
        self.created_prs.lock().unwrap().push((
            branch.to_string(),
            target.to_string(),
            title.to_string(),
            body.to_string(),
        ));
        Ok(format!("Mock PR created for {}", branch))
    }
    
    fn update_pr_base(&self, branch: &str, new_base: &str) -> Result<()> {
        if !self.existing_prs.get(branch).copied().unwrap_or(false) {
            return Err(YggitError::PullRequest(format!("PR not found for branch {}", branch)));
        }
        
        self.updated_prs.lock().unwrap().push((
            branch.to_string(),
            new_base.to_string(),
        ));
        Ok(())
    }
}