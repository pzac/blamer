# AGENTS.md

This file provides guidance to LLM agents when working with code in this repository.

## Project Overview

`blamer` is a terminal user interface (TUI) application for viewing git blame information interactively. It displays line-by-line blame data with commit information and allows users to inspect detailed commit information through a popup interface.

## Build and Run

```bash
# Build the project
cargo build

# Build optimized release version
cargo build --release

# Run the application (provide a file in a git repository)
cargo run -- <filepath>

# Example
cargo run -- src/main.rs

# Run release binary
./target/release/blamer <filepath>
```

## Architecture

The application is a Rust binary.

## Error Handling

The application exits early with error messages for:
- File does not exist
- File is not in a git repository
- Cannot get blame information (bad relative path, file not tracked, etc.)

Uncommitted changes are handled gracefully rather than erroring.
