# Blamer

This is a CLI application to help you investigate the history of a file, following its git history.

It's been built with extensive LLM help.

![screenshot](screenshot.png)

## Installation

Requires [Rust](https://www.rust-lang.org/tools/install).

```sh
cargo build --release
```

The binary will be at `target/release/blamer`. You can copy it somewhere on your `$PATH`:

```sh
cp target/release/blamer ~/.local/bin/
```

### Homebrew

If you prefer homebrew:

```
brew tap pzac/tap
brew install blamer
```

Formula is not a cask, it will compile the package and requires rust.

## Usage

```sh
blamer <file>
```

### Key bindings

| Key | Action |
|-----|--------|
| `↑` / `↓` or `j` / `k` | Scroll up / down |
| `←` / `→` | Go back / forward in history |
| `l` | Show file commit log |
| `Space` | Show commit details for the selected line |
| `PgUp` / `PgDn` | Page up / down |
| `Home` / `End` | Jump to first / last line |
| `q` / `Esc` | Quit |
