# Quick Commit

Stages all changes and auto generates a commit (and branch if desired) message and pushes.

## Install

```bash
cargo install quick-commit
```

## Usage

```bash
qc
```

## Example

```
quick-commit
M src/main.rs
+ src/config.rs
- old_file.rs
3 files changed, 45 lines added, 12 lines deleted

Create new branch? (y/N): y
Commit: feat: add user authentication endpoint
Branch: feat/user-auth-endpoint
Push to remote? (Y/n): Y
Pushed to: https://github.com/user/repo.git
```
