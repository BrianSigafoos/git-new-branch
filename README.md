# git-new-branch (gnb)

**Create git branches with username prefix.**

One command to create consistently named and sequential branches: `gnb` → `username/YYMMDD`

## Installation

### Quick Install (Recommended)

Install the latest release with a single command:

```bash
curl -LsSf https://gnb.bfoos.net/install.sh | bash
```

This downloads the prebuilt binary for your platform (macOS Apple Silicon/Intel or Linux).

### Manual Download

Binaries available on [GitHub Releases](https://github.com/BrianSigafoos/git-new-branch/releases).

### From Source

```bash
cargo install --git https://github.com/BrianSigafoos/git-new-branch
```

## Usage

```bash
# Create branch with today's date: username/YYMMDD
gnb

# Create branch with ticket number: username/ABC-123
gnb ABC-123

# Create branch with description: username/fix-login-bug
gnb fix login bug

# Multiple words become dashes
gnb "my new feature"
```

### Branch Collision Handling

If a branch already exists, a numeric suffix is automatically added:

```bash
gnb ABC-123   # → username/ABC-123
gnb ABC-123   # → username/ABC-123_2
gnb ABC-123   # → username/ABC-123_3
```

### Custom Prefix

Override the username with the `GNB_PREFIX` environment variable:

```bash
GNB_PREFIX=ci-bot gnb
# → ci-bot/YYMMDD
```

## How It Works

1. Detects your system username (or uses `GNB_PREFIX`)
2. Sanitizes the branch name (spaces → dashes, removes invalid chars)
3. Checks local and remote branches for collisions
4. Adds `_2`, `_3`, etc. suffix if branch exists
5. Creates and switches to the new branch

## Examples

| Command              | Result                |
| -------------------- | --------------------- |
| `gnb`                | `username/241215`     |
| `gnb ABC-123`        | `username/ABC-123`    |
| `gnb fix login`      | `username/fix-login`  |
| `gnb "my feature"`   | `username/my-feature` |
| `GNB_PREFIX=bot gnb` | `bot/241215`          |

---

## Development

Contributions welcome.

### Setup

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Verify
rustc --version
cargo --version
```

### Build and Run

```bash
# Build debug version
cargo build

# Run directly
cargo run
cargo run -- --help
cargo run -- ABC-123

# Build optimized release
cargo build --release
```

### Development Commands

```bash
# Check code compiles
cargo check

# Run tests
cargo test

# Format code
cargo fmt

# Lint code
cargo clippy
```

### Releasing

Releases are managed with [cargo-release](https://github.com/crate-ci/cargo-release).

```bash
# Install cargo-release (one-time)
cargo install cargo-release

# Release a new version
cargo release patch  # 0.1.0 → 0.1.1
cargo release minor  # 0.1.0 → 0.2.0
cargo release major  # 0.1.0 → 1.0.0

# Dry run
cargo release patch --dry-run
```

The push triggers the GitHub Actions release workflow.
