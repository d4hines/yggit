use thiserror::Error;

#[derive(Error, Debug)]
pub enum YggitError {
    #[error("Git operation failed: {0}")]
    Git(String),
    
    #[error("GitHub CLI operation failed: {0}")]
    GitHubCli(String),
    
    #[error("GitHub CLI not found")]
    GitHubCliNotFound,
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Branch '{0}' not found")]
    BranchNotFound(String),
    
    #[error("Pull request operation failed: {0}")]
    PullRequest(String),
    
    #[error("File operation failed: {0}")]
    File(String),
}

pub type Result<T> = std::result::Result<T, YggitError>;