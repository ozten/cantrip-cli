# cantrip-cli - Claude Context

## What This Is

Thin CLI client for the Cantrip daemon. Translates clap commands into `{command, args, flags}` JSON and POSTs to the daemon over HTTP. Zero business logic.

## Tech Stack

- Rust, clap 4, ureq 3, serde

## Structure

```
src/
  main.rs       # Entry point, request building, HTTP client
  cli/mod.rs    # Clap command definitions
  output.rs     # JSON/human/markdown output formatting
```

## How It Works

Every command maps to a `{command, args, flags}` JSON envelope POSTed to `http://127.0.0.1:9876/api/cantrip`. The daemon (`cantrip-server` in the `cantrip` repo) does all the work.

## Related Repos

- `../cantrip` — Server (daemon + domain logic + docs)
- `../cantrip-dashboard` — Next.js frontend
- `../mcp-server-cantrip` — MCP server (TypeScript, same HTTP client pattern)

## Quality Gates

```bash
cargo build
cargo clippy
```
