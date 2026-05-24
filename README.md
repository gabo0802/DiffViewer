# DiffViewer

DiffViewer is a Tauri + React desktop diff tool for Git, Perforce, patch files, and external two-file comparisons. The UI is built around Monaco editors, with Rust owning filesystem access, SCM commands, diff parsing, rendering, persistence, and merge-save behavior.

## Development

```bash
npm install
npm run build
cd src-tauri
cargo test
```

Use `npm.cmd run tauri:dev` for flows that depend on Tauri commands or local filesystem access. Use `npm.cmd run dev` only for browser-only UI iteration.

## Architecture

- `src-tauri/src/main.rs` wires app state, startup open requests, and Tauri command registration.
- `src-tauri/src/commands/` contains thin Tauri command adapters.
- `src-tauri/src/services/` contains reusable app behavior such as render, merge, and open-request handling.
- `src-tauri/src/content_source.rs` defines typed JSON models for content sources and write targets.
- `src-tauri/src/scm.rs` owns provider import/refresh behavior, while `src-tauri/src/scm/` contains reusable SCM support for P4 config and process execution.
- `src-tauri/src/diff_engine/` contains parsing, two-way diffing, alignment, rendering, and merge primitives.
- `src-tauri/src/store/` owns SQLite schema and persistence helpers.
- `src/hooks/` contains reusable React state and coordination hooks.
- `src/diffDomain.ts` centralizes frontend parsing for backend JSON fields.

## Verification

Before merging feature work, run:

```bash
cd src-tauri
cargo test
cd ..
npm.cmd run typecheck
npm.cmd run build
```

The frontend build currently emits Vite's CJS API deprecation warning from the toolchain, but the build succeeds.
