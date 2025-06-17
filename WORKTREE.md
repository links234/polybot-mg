# Git Worktree Management for Polybot

This document explains how to use the built-in `worktree` command for managing multiple feature branches with automatic data and environment setup.

## What are Git Worktrees?

Git worktrees allow you to have multiple working directories for the same repository, each checked out to different branches. This is perfect for:

- Working on multiple features simultaneously
- Testing different branches without losing work
- Comparing implementations side-by-side
- Running different experiments with separate data

## Quick Start

### List Current Worktrees
```bash
cargo run -- worktree list
```

This shows all worktrees with data directory sizes, environment files, and credential status.

### Create a New Worktree
```bash
# Create a new feature branch worktree
cargo run -- worktree create my-feature

# Create from a specific base branch
cargo run -- worktree create my-feature --base develop

# Create with custom path
cargo run -- worktree create my-feature --path /path/to/custom/location

# Create without copying data (faster)
cargo run -- worktree create my-feature --no-data
```

### Sync Data Between Worktrees
```bash
# Sync everything from main worktree
cargo run -- worktree sync --what all

# Sync only data directory
cargo run -- worktree sync --what data

# Sync only credentials
cargo run -- worktree sync --what creds

# Sync from specific worktree
cargo run -- worktree sync --source ../polybot-main --what all
```

### Remove a Worktree
```bash
# Remove cleanly (checks for uncommitted changes)
cargo run -- worktree remove ../polybot-my-feature

# Force removal (ignores dirty state)
cargo run -- worktree remove ../polybot-my-feature --force
```

## What Gets Copied

When creating a new worktree, the following are automatically copied:

### ✅ Data Directory (`data/`)
- **Portfolio snapshots** - Your trading history and positions
- **Market data** - Cached market information and analysis
- **Datasets** - Processed market data files
- **Logs** - Application logs and debugging info

### ✅ Environment Files
- `.env` - Main environment configuration
- `.env.local` - Local overrides
- `.env.production` - Production settings
- `.env.example` - Template file

### ✅ Credentials (`data/auth/`)
- **API credentials** - Encrypted Polymarket API keys
- **Private keys** - Encrypted wallet private keys
- **Authentication tokens** - Cached auth data

## Typical Workflows

### Feature Development
```bash
# 1. Create feature worktree
cargo run -- worktree create new-portfolio-widget

# 2. Switch to new worktree
cd ../polybot-new-portfolio-widget

# 3. Work on your feature
cargo run -- portfolio
# ... make changes ...

# 4. Test your changes
cargo test
cargo run -- portfolio --text

# 5. Commit when ready
git add .
git commit -m "Add new portfolio widget"

# 6. Return to main and clean up when done
cd ../polybot
cargo run -- worktree remove ../polybot-new-portfolio-widget
```

### Experiment with Different Data
```bash
# Create worktree without copying data
cargo run -- worktree create experiment --no-data

cd ../polybot-experiment

# Start fresh or sync specific data
cargo run -- init  # Fresh credentials
# OR
cargo run -- worktree sync --what creds  # Copy existing creds
```

### Compare Implementations
```bash
# Keep main worktree running
cd polybot
cargo run -- stream &  # Background process

# Work in feature branch
cd ../polybot-new-feature
cargo run -- stream  # Different implementation

# Compare side-by-side
```

## Directory Structure

After creating worktrees, your directory structure might look like:

```
work/
├── tp/
│   ├── polybot/                    # Main worktree (main branch)
│   │   ├── data/                   # 564 MB of data
│   │   ├── .env
│   │   └── src/
│   ├── polybot-portfolio-upgrade/  # Feature worktree
│   │   ├── data/                   # Copied from main
│   │   ├── .env                    # Copied from main
│   │   ├── WORKTREE.md            # Auto-generated guide
│   │   └── src/
│   └── polybot-experiment/         # Experiment worktree
│       ├── data/                   # Independent data
│       └── src/
```

## Advanced Usage

### Custom Sync Strategies

```bash
# Sync only recent portfolio data
cargo run -- worktree sync --what data
# Then manually clean old data if needed

# Sync everything except credentials (use fresh auth)
cargo run -- worktree sync --what data
cargo run -- worktree sync --what env
# Skip creds sync, run: cargo run -- init

# Sync from non-main worktree
cargo run -- worktree sync --source ../polybot-experiment --what data
```

### Data Management

```bash
# Check data sizes across worktrees
cargo run -- worktree list

# Clean up large datasets in specific worktrees
cd ../polybot-experiment
rm -rf data/datasets/*  # Remove large market data files

# Sync fresh data when needed
cargo run -- worktree sync --what data
```

### Development Tips

1. **Use meaningful branch names** - They become directory names
2. **Commit regularly** - Worktrees make it easy to switch contexts
3. **Sync data periodically** - Keep portfolio data up to date
4. **Clean up unused worktrees** - They consume disk space
5. **Use `--no-data` for quick experiments** - Skip copying large datasets

## Troubleshooting

### "Worktree path already exists"
```bash
# Remove the directory manually if it's stale
rm -rf ../polybot-my-feature
git worktree prune  # Clean up git's worktree list
```

### "Source worktree not found"
```bash
# List worktrees to see available sources
cargo run -- worktree list

# Specify source explicitly
cargo run -- worktree sync --source ../polybot --what data
```

### "Permission denied" during copy
```bash
# Check file permissions
ls -la data/auth/

# Re-run with proper permissions
chmod -R 755 data/
```

### Large data directory sync is slow
```bash
# Sync only what you need
cargo run -- worktree sync --what creds  # Just credentials
cargo run -- worktree sync --what env    # Just environment

# Or create without data and sync later
cargo run -- worktree create my-branch --no-data
cd ../polybot-my-branch
cargo run -- worktree sync --what creds
```

## Integration with Git

The `worktree` command is built on top of `git worktree` and provides:

- ✅ **Automatic branch creation** from base branch
- ✅ **Data directory management** with size tracking
- ✅ **Environment file copying** with validation
- ✅ **Credential management** with encryption preservation
- ✅ **Cleanup utilities** with safety checks
- ✅ **Status overview** with visual indicators

All standard git operations work normally in each worktree:
- `git status`, `git commit`, `git push`
- `git merge`, `git rebase`, `git cherry-pick`
- `git branch`, `git checkout`, `git log`

The worktrees share the same git history but have independent working directories and can be on different branches simultaneously.