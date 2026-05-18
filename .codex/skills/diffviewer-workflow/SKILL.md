---
name: diffviewer-workflow
description: Repo-local workflow for the DiffViewer Tauri app. Use when working in this repository on Git/P4 diff import behavior, Monaco diff/merge UI, Tauri command flows, Perforce `.p4config` handling, or when continuing the feature-by-feature commit pattern already established here.
---

# DiffViewer Workflow

Use the existing repo patterns rather than inventing new ones.

## Core Workflow

1. Read the current implementation before changing behavior.
2. Keep edits scoped to the feature or bug at hand.
3. Verify with:
   - `cargo test` in `src-tauri`
   - `npm.cmd run build` in the repo root
4. If you run a formatter or lint-like repo-wide command such as `cargo fmt`, say so explicitly in updates and in the final summary.
5. Prefer formatting only touched Rust files when practical, because `cargo fmt` can rewrite unrelated files in the worktree.
6. If formatting changes spill into unrelated files, do not stage or commit them silently; call them out explicitly.
7. When a feature-sized change is complete, create a focused git commit.

## Runtime Rules

- Use `npm.cmd run tauri:dev` when the task depends on Tauri commands, local filesystem access through the desktop app, or Perforce/Git import actions.
- Use `npm.cmd run dev` only for browser-only UI iteration. The plain Vite page does not provide the Tauri bridge, so `invoke`-based actions will fail there.
- If a user reports `window.__TAURI_INTERNALS__ is undefined`, assume they are in the browser runtime instead of the Tauri desktop runtime.

## Perforce Rules

- The workspace path supplied by the user is the anchor for Perforce context.
- Search upward from that path for `.p4config`.
- Use `.p4config` values to set subprocess env for Perforce commands:
  - `P4CLIENT`
  - `P4PORT`
  - `P4USER`
  - `P4CHARSET`
- If `P4CHARSET` is missing, default it to `utf8`.
- Keep pending changelists workspace-scoped. Do not treat `default` as depot-global.

## Perforce Command Patterns

- Pending changelists:
  - use `p4 opened -c <change>` when `.p4config` gives a client
  - use `p4 diff -du` for actual pending file diffs
  - chunk large file lists to avoid Windows `os error 206`
- Shelved changelists:
  - use `p4 describe -S -du <change>`
- Submitted changelists:
  - use `p4 describe -du <change>`
- Submitted and shelved diffs are read-only in the UI.

## P4 Debugging

- Base Debugging lives in `src-tauri/src/debugging.rs`.
  - Enable it by passing `--debug` when running the app.
- Perforce logging lives in `src-tauri/src/scm.rs`.
- Look for `[diffviewer-debug][scm]` and `[diffviewer-debug][merge]` lines in the Tauri dev console when debugging backend diff state.
- When debugging P4 failures, inspect:
  - chosen `config_path`
  - `client`
  - `port`
  - `user`
  - command args
  - stdout
  - stderr
- If P4 behavior is wrong, fix the backend command/context first before adjusting the UI.

## UI Patterns

- Main coordination files:
  - `src/App.tsx`
  - `src/components/Sidebar.tsx`
  - `src/components/DiffViewer.tsx`
  - `src/components/MergePanel.tsx`
  - `src/styles/global.css`
- Backend SCM/diff logic:
  - `src-tauri/src/scm.rs`
  - `src-tauri/src/diff_engine/*`
  - `src-tauri/src/main.rs`
- Reuse the in-app dialog pattern instead of raw `window.prompt` for structured input.
- Keep diff and merge behavior aligned with desktop diff-tool expectations:
  - synchronized scrolling
  - clear add/delete highlighting
  - read-only handling where appropriate

## Diff Safety Rules

- Do not write reconstructed patch fragments back to real files.
- For live Git/P4 editable diffs, ensure save targets point at real workspace files and content sources come from real file content or depot content as appropriate.
- If a bad save corrupts a file and the change was never committed, prefer a local restore over ad hoc manual reconstruction when safe to do so.

## Git Workflow

- The user asked for feature-by-feature commits in this repo. Preserve that pattern unless explicitly told otherwise.
- Keep commits narrow and descriptive.
- Do not stage unrelated user scratch changes.
- Assume the worktree may contain intentional local edits; never revert them unless asked.

## Good Defaults

- Prefer backend fixes over frontend workarounds when import/render data is wrong.
- Prefer reusable UI components over one-off prompts and alerts when a flow is likely to recur.
- Preserve the existing provider-aware model:
  - Git
  - P4
  - patch
  - external
