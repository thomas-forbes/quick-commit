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

Use `-v` or `--verbose` to print AI query metrics after the response comes back:

```bash
qc -v
```

## Configuration

Quick Commit stores settings in `~/.config/quick-commit/config.toml`.
On first run, you'll be prompted for your API key and model.

Example `config.toml`:

```toml
api_key = "your-api-key"
model = "openai/gpt-oss-20b"
```

## Example

```
quick-commit
M src/main.rs
+ src/config.rs
- old_file.rs
R utils.rs → helpers.rs
3 files changed, 45 lines added, 12 lines deleted

Create new branch? (y/N): y
Commit: feat: add user authentication endpoint
Branch: feat/user-auth-endpoint
Push to remote? (Y/n): Y
Pushed to: https://github.com/user/repo.git
```
