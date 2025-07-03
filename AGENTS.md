# AGENTS.md

## Introduction
Yggit is a specialized tool designed to support a "stacked commit" git workflow. It streamlines the process of creating layered commits, improving review efficiency and maintaining a clear commit history.

## Repository Structure
The repository is organized to facilitate clear separation of concerns:

- **Cargo Files**
  - `Cargo.toml` and `Cargo.lock`: Define project dependencies and configurations for this Rust project.
  
- **Source Code (`src/`)**
  - **Core Functionality:**  
    - `src/core.rs`: Contains the fundamental implementations of Yggit.
  - **Commands:**  
    - `src/commands/`: Houses command implementations such as `push.rs` and `show.rs`.
  - **Git Integration:**  
    - `src/git/`: Contains modules for git operations including `git.rs` and `config.rs`.
  - **Parsing:**  
    - `src/parser/`: Includes grammar definitions (e.g., `yggit.pest`) and parsing logic.
    
- **Editor Integration (`editor/`)**
  - **Neovim Integration:**
    - Files under `editor/nvim/` provide integration support.
    - See `editor/nvim/install.md` for installation and usage instructions.

- **Documentation**
  - `README.md`: Offers general information about the project.
  - This file (AGENTS.md) provides a detailed guide for future contributors.

## Development Workflow
To work effectively with Yggit, follow these steps:

- **Building the Project:**  
  Use `cargo build` to compile the project.

- **Running the Application:**  
  Execute with `cargo run -- [arguments]` as necessary.

- **Testing:**  
  Run tests using `cargo test` to ensure code reliability.

- **Code Quality:**  
  Format your code with `cargo fmt` and check for linting issues using `cargo clippy`.

## The Stacked Commit Workflow
Yggit implements a "stacked commit" strategy to improve collaboration and code management:

- **Commit Layering:**  
  Each change is isolated into separate commits, making code review simpler and history cleaner.

- **Key Commands:**  
  Refer to `src/commands/push.rs` for details on how push operations are handled within this workflow.

- **Best Practices:**  
  - Keep commits small and focused.
  - Use descriptive commit messages.
  - Regularly review commit stacks to maintain clarity.

## Editor Integration
Yggit comes with dedicated support for Neovim:

- **Setup:**  
  Follow the instructions in `editor/nvim/install.md` to install and configure Neovim integration.

- **Usage:**  
  Ensure your editor is configured to work with Yggit's features. Additional integrations may be added in the future.

## Contribution Guidelines and Testing
To maintain high-quality contributions:

- **Testing:**  
  Run `cargo test` before pushing changes.

- **Formatting and Linting:**  
  Always run `cargo fmt` and `cargo clippy` to ensure code consistency and quality.

- **Documentation Updates:**  
  Update AGENTS.md and README.md when new processes or features are introduced.

- **General Best Practices:**  
  - Follow the stacked commit workflow diligently.
  - Keep code changes isolated.
  - Maintain clear, descriptive commit messages and update the documentation as necessary.

---

This guide is intended to ensure that future contributors have everything they need to work efficiently on the Yggit repo. Stay updated with any repository changes and contribute to ongoing improvements.
