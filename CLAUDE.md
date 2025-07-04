# Yggit - Claude Context

## Project Overview
Yggit is a Git workflow tool that manages DAG (Directed Acyclic Graph) branch structures and integrates with GitHub for automated PR management.

## Core Concepts

### DAG Branch Management
- Users edit commits in a temporary file with branch targets
- Syntax: `-> <branch>` or `-> <branch> => <parent_branch>`
- Implicit parent chains: branches inherit from previous branch unless explicitly specified
- Example workflow:
  ```
  commit1 -> feature-1
  commit2 -> feature-2 => main  
  commit3 -> feature-3          # inherits from feature-2
  ```

### GitHub Integration
- Automatic PR creation/updates based on branch state changes
- Compares "before" and "after" states to determine actions
- Uses GitHub CLI (`gh`) for PR operations
- Handles target branch changes and missing PRs

## Architecture

### Key Files
- `src/main.rs`: CLI entry point with logging initialization (44 lines)
- `src/commands/push.rs`: Main push command, now GitHub-integration free (85 lines)
- `src/core.rs`: Core Git operations and DAG processing with proper error handling (502 lines)
- `src/git/git.rs`: Git repository operations with structured errors (394 lines)
- `src/parser.rs`: Commit instruction parsing (312 lines)
- `src/errors.rs`: Structured error types using thiserror (33 lines)
- `src/github/`: GitHub integration module
  - `mod.rs`: Module exports and test configuration
  - `types.rs`: BranchState and related types
  - `cli.rs`: GitHub CLI abstraction with mock implementation
  - `integration.rs`: Core GitHub PR management logic
  - `tests.rs`: Comprehensive test suite (300+ lines)

### Data Flow
1. `git.list_commits()` → captures current state
2. User edits temp file with branch instructions
3. `parser::instruction_from_string()` → parses new state
4. `save_note()` → saves branch targets to Git notes
5. `push_from_notes()` → creates branches with DAG relationships
6. `GitHubIntegration::handle_integration()` → manages PRs based on state diff

## Testing Strategy

### Test Coverage (23 tests total)
- **Core DAG Operations**: Comprehensive tests for branch creation and parent relationships
- **GitHub Integration**: 9 new tests covering all PR management scenarios
  - New branch creation
  - Target branch changes  
  - Missing PR detection
  - CLI availability checks
  - Complex workflow scenarios
- **Branch State Management**: Tests for state extraction and comparison
- **Mock Infrastructure**: `MockGitHubCli` for testing without external dependencies
- **Property-based Testing**: Complex DAG scenarios and edge cases

## Recent Major Refactoring (07/03/2025)
- **GitHub Integration Extraction**: Moved 290+ lines from push.rs to dedicated `src/github/` module
- **Error Handling Overhaul**: Replaced `Result<(), ()>` with structured `YggitError` using thiserror
- **GitHub CLI Abstraction**: Created testable `GitHubCli` trait with mock implementation
- **Comprehensive Testing**: Added 9 tests covering all GitHub integration scenarios
- **Logging Infrastructure**: Replaced println! statements with structured logging
- **Dependency Injection**: GitHub integration now uses dependency injection for testability

## Common Commands
- `cargo test` - Run existing tests
- `cargo run -- show` - show the current yggit state of this repo

## Dependencies
- `clap` - CLI parsing
- `git2` - Git operations
- `serde*` - JSON serialization for Git notes
- `regex` - Parsing
- `auth-git2` - Git authentication
- `thiserror` - Structured error handling
- `log` - Structured logging
- `env_logger` - Environment-based log configuration

## Error Handling Patterns
- **Structured Errors**: All functions use `Result<T, YggitError>` with context
- **Error Propagation**: Proper error bubbling with `?` operator
- **User-Friendly Messages**: Errors include helpful context and suggestions
- **Error Categories**: Git, GitHub CLI, Parse, IO, Branch, and PR-specific errors

## Observability
- **Structured Logging**: Uses `log` crate with configurable levels
- **GitHub Operations**: Info-level logs for PR creation/updates
- **Error Tracking**: Error-level logs with full context
- **Debug Information**: Debug-level logs for troubleshooting
- **Environment Configuration**: Set `RUST_LOG=debug` for verbose output

## Testing and Development

### Running Tests
```bash
cargo test                    # Run all 23 tests
cargo test github::           # Run GitHub integration tests only
cargo test core::             # Run core DAG operation tests
RUST_LOG=debug cargo test    # Run tests with debug logging
```

### Code Architecture
- **Separation of Concerns**: GitHub logic isolated from core Git operations
- **Testable Design**: Dependency injection enables comprehensive testing
- **Error Transparency**: Structured errors provide clear failure context
- **Maintainable Structure**: Each module has clear responsibilities