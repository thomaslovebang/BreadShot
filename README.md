# FocusFlow

FocusFlow is a Windows-first, local desktop tool that turns ordinary screen recordings into polished product demos. It records the screen and the mouse, builds a virtual-camera timeline, and exports a final MP4 with FFmpeg.

Phase 9 is the current product stage. It adds a manual workbench for fine-tuning that camera path. The Phase 6 automatic record-to-final-MP4 pipeline still runs on its own.

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

## What the workbench does

After a recording finishes, or after you open a `.focusflow` project, the workbench replaces the recording screen.

- Preview with play/pause, scrubbing, and a switch between the rendered final MP4 and the source recording.
- A multi-track timeline: Video, Camera, and an Audio placeholder. The audio track is not editable yet; the export still keeps the source audio.
- Camera keyframes drawn on the camera track. Drag a handle horizontally to change its time, or vertically to change zoom. On the source preview, drag the focus handle to pan.
- Add a keyframe at the playhead, delete an interior keyframe, reset one keyframe to the automatic pose, undo the last camera edit, or restore the whole automatic camera.
- **导出镜头** writes the edited camera into the project edit file and re-exports the final MP4 with the existing FFmpeg renderer. The original `camera.json` from the automatic director is left in place.
- Video-track split, move, and ripple delete still work. **生成片段** re-exports those cuts and realigns the camera to the automatic path.

You can ignore the workbench. Stop a recording and FocusFlow still writes the source MP4, the mouse trace, the camera timeline, and the final MP4 by itself.

## Workspace

- `apps/desktop`: Tauri and React desktop shell
- `crates/domain`: shared domain models
- `crates/recorder`: recording contracts and session lifecycle
- `crates/tracker`: mouse tracking contracts
- `crates/focus_engine`: automatic camera timeline generation
- `crates/editor_core`: clip edits and manual camera keyframes
- `crates/renderer`: FFmpeg video rendering
- `crates/media`: FFmpeg/FFprobe integration boundary
- `crates/project_store`: project and edit-timeline persistence
- `crates/platform_windows`: Windows-specific adapters
- `docs`: architecture, data contracts, and historical phase notes

## Scope

V1 is Windows-only. Processing stays on the machine: no cloud services, accounts, or watermark.

Older phase write-ups under `docs/` are historical test and design notes. Phase 9 is current.
