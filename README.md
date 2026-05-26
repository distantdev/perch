# Perch

[![CI](https://github.com/distantdev/perch/actions/workflows/ci.yml/badge.svg)](https://github.com/distantdev/perch/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

Perch is a cross-platform CLI and TUI for **local development servers**. It reads OS socket tables, maps ports to processes, and lets you inspect, filter, and control listeners without changing your project scripts.

## Why Perch

When several dev servers run at once (Node, .NET, Python, Docker, etc.), ports pile up and orphaned processes linger. Perch answers:

- What is listening on port 3000 (or 5173, 8080, ...)?
- Which project directory owns that process?
- How do I stop or freeze it from the terminal?

No background agent. No changes to `package.json` or `launchSettings.json`.

## Install

### Prebuilt binaries (recommended)

Download the archive for your platform from [GitHub Releases](https://github.com/distantdev/perch/releases), unpack, and put `perch` (or `perch.exe`) on your `PATH`.

### From source

Requires [Rust](https://rustup.rs/) 1.78+.

```bash
git clone https://github.com/distantdev/perch.git
cd perch
cargo install --path .
```

Or build once:

```bash
cargo build --release
# binary: target/release/perch (or perch.exe on Windows)
```

## Quick start

```bash
# Interactive dashboard (default)
perch

# Table of dev listeners
perch list

# JSON for scripts
perch list --format json

# Stop whatever owns port 3000
perch kill 3000
```

By default Perch shows **loopback TCP** listeners for **dev-like processes** (Node, .NET, Python, Docker, etc.). Use `--all` to include wildcard binds (`0.0.0.0`), UDP, and other local listeners.

## Commands

| Command | Description |
|---------|-------------|
| `perch` | Interactive TUI |
| `perch ui` | Same as above |
| `perch list` / `perch ls` | Table output |
| `perch list --format json` | JSON output |
| `perch kill <port\|pid>` | Graceful stop |
| `perch kill <port\|pid> -f` | Force stop |
| `perch pause <port\|pid>` | Freeze process |
| `perch resume <port\|pid>` | Resume paused process |

Global flags: `--config <path>`, `-v` / `--verbose`, `--all`.

## TUI keys

| Key | Action |
|-----|--------|
| Up / Down | Move selection |
| `/` or `f` | Edit filter (header) |
| `s` | Cycle sort |
| `x` | Kill (Shift+`K` also works) |
| `X` | Force kill (Shift+`F` also works) |
| `p` | Pause (Shift+`P` also works) |
| `u` | Resume (Shift+`C` also works) |
| `q` | Quit |

The footer shows the selected row's command line when available.

## Configuration

Default file: `~/.config/perch/config.toml` (created from [config.toml.example](config.toml.example) on first run).

**`[scan]`** (dev-focused defaults):

| Option | Default | Meaning |
|--------|---------|---------|
| `loopback_only` | `true` | Only `127.0.0.1` / `::1` |
| `tcp_only` | `true` | Hide UDP |
| `dev_only` | `true` | Hide non-dev processes |
| `include_wildcard_bind` | `false` | Also show `0.0.0.0` / `::` |

**`[filter]`**: `ignore_ports`, `ignore_executables`, `ignore_cmdline_patterns`.

Vite/Next often bind to `0.0.0.0`. Set `include_wildcard_bind = true` if you need those in the default view.

## Platform support

| OS | Socket scan | Process control |
|----|-------------|-----------------|
| Windows | `GetExtendedTcpTable` | `TerminateProcess`, `NtSuspendProcess` |
| Linux | `/proc/net/tcp{,6}` | POSIX signals |
| macOS | `lsof` + `proc_pidpath` | POSIX signals |

You may need elevated rights to control processes owned by another user.

## Releasing

Releases are built automatically when you push a **version tag**:

1. Bump `version` in [Cargo.toml](Cargo.toml).
2. Commit and push to `main`.
3. Tag and push:

```bash
git tag v0.1.0
git push origin v0.1.0
```

The [Release](.github/workflows/release.yml) workflow builds binaries for Linux, Windows, and macOS (Apple Silicon) and attaches them to a GitHub Release. Tag names must start with `v` (for example `v0.1.0`).

## Development

```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
cargo build --release
```

Design notes: [SPEC.md](SPEC.md).

## Security

Perch only controls processes your user can open or signal. Other users' processes return a clear error.

## License

[MIT](LICENSE)
