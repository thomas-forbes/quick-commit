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

## Configuration

Quick Commit stores settings in `~/.config/quick-commit/config.toml` and
also reads environment variables for overrides.

Environment variables:

- `OPENROUTER_API_KEY`
- `OPENROUTER_MODEL`

Example `config.toml`:

```toml
api_key = "your-api-key"
model = "openai/gpt-oss-120b"
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
