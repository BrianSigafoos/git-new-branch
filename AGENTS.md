# AGENTS.md

## Project Overview

git-new-branch (gnb) is a Rust CLI creates consistently named and sequential git branches `gnb` → `username/YYMMDD`

## Development Commands

```bash
# Build the project
cargo build

# Run tests
cargo test

# Quick compilation check
cargo check
```

## Formatting

Run `ffx` to auto-format all files after every code change. Don't manually format code.

## Testing

Add a test for every code change.

## Rust code principles

- Keep functions small and single-purpose; extract helpers instead of growing long flows.
- Prefer immutable data and borrowed references; only clone or allocate when intent is explicit.
- Capture fallible operations with clear context via `anyhow::Context`; avoid `unwrap`/`expect` outside tests.
- Use focused types and constructors to represent states instead of repeating literal structs or flags.
- Favour deterministic, readable control flow (early returns, explicit naming, predictable ordering) over cleverness.
