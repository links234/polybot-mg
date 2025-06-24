# Polybot Worktree: grpc

This is a git worktree for the `grpc` branch, created from `main`.

## Quick Start

```bash
# Install dependencies
cargo build

# Run portfolio command
cargo run -- portfolio

# Run any command with help
cargo run -- --help
```

## Worktree Management

```bash
# List all worktrees
cargo run -- worktree list

# Sync data from main worktree
cargo run -- worktree sync --what all

# Remove this worktree
cargo run -- worktree remove /Users/alexandrumurtaza/work/tp/polybot-grpc
```

## Data Structure

- `data/` - Application data (copied from main worktree)
- `.env*` - Environment files (copied from main worktree)
- `data/auth/` - API credentials (copied from main worktree)

## Important Notes

- This worktree shares git history but has independent working directory
- Data and credentials are copied, not linked
- Use `cargo run -- worktree sync` to update data from main branch
- Commit changes regularly to avoid conflicts

Created: 2025-06-23 20:23:30 UTC
Branch: grpc
Base: main
