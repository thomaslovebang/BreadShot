# FocusFlow

FocusFlow is a focused Windows screen recorder. It records the screen and mouse locally, turns mouse actions into automatic zoom and pan, and exports a watermark-free MP4.

The current product has one workflow: choose a display, save folder, and start/stop shortcut; record for as long as needed; then follow the local processing stages and play the automatically directed video.

## Run the desktop app

From `apps/desktop` on Windows:

```powershell
npm install
npm run tauri dev
```

Check the Rust workspace from the repository root:

```powershell
cargo check --workspace
```

FFmpeg has to be available as `ffmpeg.exe` beside the app, under `apps/desktop/src-tauri/bin`, in WinGet (`Gyan.FFmpeg.Essentials`), or on `PATH`. `ffprobe` is resolved next to that FFmpeg binary.

## Current product scope

- No artificial recording time limit.
- No watermark.
- Automatic zoom and pan generated from mouse clicks and movement.
- Select one display to record on multi-monitor systems.
- User-selected output folder.
- One configurable global shortcut that starts and stops recording.
- Visible save, mouse-analysis, and automatic-camera rendering stages.
- A finished-video result with Play Video and Open Folder actions.
- Recoverable error feedback that keeps the saved source recording accessible.

Manual timelines, project opening, clip editing, camera keyframes, and director-style settings are outside the current product.

## Workspace

- `apps/desktop`: Tauri and React desktop shell
- `crates/domain`: shared domain models
- `crates/recorder`: recording contracts and session lifecycle
- `crates/tracker`: mouse tracking contracts
- `crates/focus_engine`: automatic camera timeline generation
- `crates/editor_core`: retained experimental editing core; not exposed in the current app
- `crates/renderer`: FFmpeg video rendering
- `crates/media`: FFmpeg/FFprobe integration boundary
- `crates/project_store`: project and edit-timeline persistence
- `crates/platform_windows`: Windows-specific adapters
- `docs`: architecture, data contracts, and historical phase notes

## Scope

V1 is Windows-only. Processing stays on the machine: no cloud services, accounts, or watermark.

Older phase write-ups under `docs/` are historical test and design notes.

