# devpulse

> Your developer environment companion — health checks, TODO scanner, and project context in one CLI tool.

```
devpulse all

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  devpulse — full project scan
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

  [1 / 3] Health check
  ───────────────────────

  ✔  Rust          rustc 1.92.0
  ✔  Cargo         cargo 1.92.0
  ✔  Git           git version 2.43.0
  ✔  Node.js       v24.10.0
  ✔  Docker        Docker version 27.0.0
  ✘  .env file     No .env found in /home/user/startup/myapp

  Result: 5/6 checks passed
  1 issue(s) need attention.

  [2 / 3] TODO scan
  ───────────────────────

  src/main.rs
    line 12     TODO   add auth middleware
    line 34     FIXME  this panics on empty input

  src/routes/user.rs
    line 8      HACK   temporary workaround for rate limiting

  Found 3 TODO(s) across 2 file(s)

  [3 / 3] Project context
  ───────────────────────

  ◆ Branch       main
  ◆ Changes      2 uncommitted change(s)
  ◆ Last commit  Add user authentication (Jane Doe)
  ◆ TODOs        3 TODO/FIXME/HACK(s) in project

  Session saved to ~/.devpulse/myapp.json

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Done. Run `devpulse --help` for individual commands.
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

---

## Why devpulse?

Every developer knows the feeling — you switch back to a project after a few days and spend the first 10 minutes just re-orienting yourself. What branch am I on? Did I leave anything broken? What was I working on? Are my tools up to date?

**devpulse solves this.** One command gives you a full snapshot of your project and environment before you write a single line of code.

---

## Features

- **Health check** — scans your machine for required developer tools (Rust, Node, Git, Docker, etc.) and reports their versions or flags them as missing
- **TODO scanner** — recursively walks your project and surfaces every `TODO`, `FIXME`, `HACK`, `NOTE`, and `XXX` comment across all source files, grouped by file with line numbers
- **Project context** — captures your current git branch, uncommitted changes, last commit message, and TODO count in a single snapshot, saved to `~/.devpulse/` for future reference
- **Run all at once** — `devpulse all` chains all three commands in sequence so you get a complete picture in one shot
- **Structured logging** — powered by `tracing`, with log levels controlled by `RUST_LOG` so operators get full visibility without polluting normal output
- **Zero panics** — every failure path is handled explicitly with typed errors via `thiserror`. No `.unwrap()` in production code paths
- **Cross-platform** — works on Linux, macOS, and Windows

---

## Installation

### From crates.io (recommended)

```bash
cargo install devpulse
```

### From source

```bash
git clone https://github.com/GOODBADBOY10/selfman.git
cd selfman
cargo install --path .
```

### Requirements

- Rust 1.75 or later
- Cargo (comes with Rust)

Install Rust via [rustup.rs](https://rustup.rs) if you don't have it.

---

## Usage

### Run everything at once

```bash
devpulse all
```

Runs health check, TODO scan, and project context in sequence. This is the recommended way to start a work session.

You can also point the TODO scan at a specific directory:

```bash
devpulse all ~/projects/my-app
```

---

### Health check

```bash
devpulse health
```

Checks whether the following tools are installed and prints their versions:

| Tool | Binary |
|------|--------|
| Rust | `rustc` |
| Cargo | `cargo` |
| Git | `git` |
| Node.js | `node` |
| Docker | `docker` |
| .env file | checks current directory |

**Example output:**
```
devpulse health check
Scanning your dev environment...

  ✔  Rust          rustc 1.92.0
  ✔  Git           git version 2.43.0
  ✘  Docker        docker not found — consider installing it
  ✘  .env file     No .env found in /home/user/myapp

Result: 3/4 checks passed
1 issue(s) need attention.
```

---

### TODO scanner

```bash
devpulse todos .               # scan current directory
devpulse todos ~/projects      # scan a specific path
```

Recursively scans source files for the following tags:

| Tag | Color | Meaning |
|-----|-------|---------|
| `TODO` | Blue | Something to implement |
| `FIXME` | Red | Something broken that needs fixing |
| `HACK` | Yellow | A workaround that should be cleaned up |
| `NOTE` | Blue | An important note for future readers |
| `XXX` | Red | A known problem or danger zone |

**Scanned file types:** `.rs`, `.ts`, `.js`, `.tsx`, `.jsx`, `.py`, `.go`, `.java`, `.c`, `.cpp`, `.h`, `.cs`, `.rb`, `.php`, `.swift`, `.kt`, `.toml`, `.yaml`, `.yml`, `.md`

**Automatically skipped:** `.git/`, `node_modules/`, `target/`, `.next/`, `dist/`, `build/`, `.cache/`

Only tags inside **comments** are matched — string literals and variable names are ignored.

**Example output:**
```
devpulse todos scan
Scanning /home/user/myapp...

  src/auth/middleware.rs
    line 12     TODO   add token expiry check
    line 45     FIXME  panics when header is missing

  src/db/connection.rs
    line 8      HACK   reconnect logic is not thread safe

Found 3 TODO(s) across 2 file(s)
```

---

### Project context

```bash
devpulse context
```

Captures a snapshot of your current project and saves it to `~/.devpulse/<project-name>.json`.

Shows:
- Current git branch
- Number of uncommitted changes
- Last commit message and author
- Total TODO/FIXME/HACK count in the project

**Example output:**
```
devpulse context
Project: /home/user/myapp

  ◆ Branch       feat/auth
  ◆ Changes      3 uncommitted change(s)
  ◆ Last commit  Add JWT middleware (Jane Doe)
  ◆ TODOs        5 TODO/FIXME/HACK(s) in project

  Session saved to ~/.devpulse/myapp.json
```

The saved session at `~/.devpulse/myapp.json` looks like this:

```json
{
  "project_name": "myapp",
  "project_path": "/home/user/myapp",
  "last_seen": "1746291600",
  "git": {
    "branch": "feat/auth",
    "uncommitted_changes": 3,
    "last_commit_message": "Add JWT middleware",
    "last_commit_author": "Jane Doe"
  },
  "todo_count": 5
}
```

---

## Debugging and logging

devpulse uses structured logging via the `tracing` crate. Log output is hidden by default and controlled by the `RUST_LOG` environment variable.

```bash
# Show all debug logs
RUST_LOG=debug devpulse health

# Show only warnings and errors
RUST_LOG=warn devpulse todos .

# Show per-check timing in the output
DEVPULSE_DEBUG=1 devpulse health
```

The `DEVPULSE_DEBUG=1` flag adds timing information to each health check line, useful for diagnosing slow tool probes (e.g. a cold Docker daemon).

---

## Project structure

```
devpulse/
├── src/
│   ├── main.rs                  # CLI entry point, subcommand routing
│   ├── error.rs                 # Typed error enum via thiserror
│   └── commands/
│       ├── mod.rs               # Module declarations
│       ├── health.rs            # devpulse health
│       ├── todos.rs             # devpulse todos
│       └── context.rs           # devpulse context
├── Cargo.toml
└── README.md
```

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `clap` | CLI argument parsing with derive macros |
| `colored` | Terminal color output |
| `walkdir` | Recursive directory traversal |
| `serde` + `serde_json` | Session serialization to JSON |
| `thiserror` | Typed, ergonomic error definitions |
| `tracing` | Structured, leveled logging |
| `tracing-subscriber` | Log formatting and `RUST_LOG` filter support |
| `dirs` | Cross-platform home directory resolution |

---

## Roadmap

- [ ] `devpulse todos --filter FIXME` — filter by tag type
- [ ] `devpulse todos --json` — machine-readable output for CI pipelines
- [ ] Port availability check in health (e.g. flag if 3000 or 8080 is occupied)
- [ ] `.env` variable completeness check against a `.env.example` file
- [ ] `devpulse context --diff` — show what changed since the last session
- [ ] Shell integration — auto-run `devpulse context` when you `cd` into a project

---

## Contributing

Contributions are welcome! Please open an issue first to discuss what you'd like to change.

```bash
# Clone and build
git clone https://github.com/GOODBADBOY10/selfman
cd selfman
cargo build

# Run with debug logs
RUST_LOG=debug cargo run -- health
RUST_LOG=debug cargo run -- todos .
RUST_LOG=debug cargo run -- context
RUST_LOG=debug cargo run -- all
```

---

## License

MIT — see [LICENSE](LICENSE) for details.

---

Built with Rust 🦀 by [Ademola](https://github.com/GOODBADBOY10)