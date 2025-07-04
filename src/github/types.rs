use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BranchState {
    pub branch: String,
    pub target_branch: String,
    pub origin: Option<String>,
    pub commit_title: Option<String>,
    pub commit_description: Option<String>,
}