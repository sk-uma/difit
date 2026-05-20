# difit-gpui

A native desktop client for [difit](../README.md) built with
[GPUI](https://www.gpui.rs/) — the GPU-accelerated UI framework that powers
[Zed](https://zed.dev).

The client is a thin GUI on top of the existing difit Node server: it speaks
the same HTTP API the React frontend uses (`/api/diff`, `/api/comments`,
`/api/watch`, …). Launch the server with `pnpm run dev` (or
`difit <target> --no-open`) and point this client at it.

## Status

This is an in-progress scaffold. Working today:

- [x] Project skeleton, CLI flag parsing, GPUI window
- [x] Typed API client matching `src/types/diff.ts`
- [x] File list + unified diff rendering
- [ ] Side-by-side view
- [ ] Syntax highlighting (syntect)
- [ ] Review comment threads
- [ ] Revision selector / PR mode plumbing
- [ ] SSE live updates (`/api/watch`, `/api/heartbeat`)

## Prerequisites

- A recent stable Rust toolchain (`rustup default stable`).
- On Windows: Visual Studio Build Tools with the "Desktop development with C++"
  workload. GPUI links against system libraries and needs the MSVC linker.
- A running difit server. The fastest way during development:

  ```bash
  pnpm install
  pnpm run dev -- --no-open
  ```

  By default the server listens on `http://localhost:4966`.

> GPUI is pulled directly from the Zed monorepo (`branch = "main"`). It moves
> fast — if the build breaks on a fresh `cargo update`, pin `gpui` in
> `Cargo.toml` to a specific `rev = "…"` that worked for you.

## Run

```bash
cd gpui-client
cargo run -- --server http://localhost:4966
```

`--server` defaults to `http://127.0.0.1:4966`, and the env var
`DIFIT_SERVER` is also honored.

## Layout

```
src/
  main.rs        # CLI parsing, GPUI Application bootstrap
  app.rs         # Root view: file list + diff viewer split
  api/
    mod.rs
    types.rs     # Rust mirrors of src/types/diff.ts
    client.rs    # reqwest-based HTTP client
  ui/
    mod.rs
    file_list.rs # Left pane: list of changed files
    diff_view.rs # Right pane: unified diff renderer
    theme.rs     # GitHub-like dark palette
```
