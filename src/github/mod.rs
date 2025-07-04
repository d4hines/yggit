pub mod cli;
pub mod integration;
pub mod types;

#[cfg(test)]
mod tests;

pub use cli::{GitHubCli, GitHubCliImpl};
pub use integration::{GitHubIntegration, extract_branch_state, extract_branch_state_from_parsed};
pub use types::BranchState;