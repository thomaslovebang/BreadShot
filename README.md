# FocusFlow

FocusFlow is a Windows-first, local desktop tool that turns ordinary screen recordings into polished product demos using mouse-driven virtual camera motion.

## V1 scope

- Screen recording
- Timestamped mouse tracking
- Mouse-driven camera timeline generation
- Local FFmpeg rendering to MP4

The application uses Tauri 2, React, TypeScript, and a modular Rust workspace. V1 is Windows-only and has no cloud services, accounts, watermark, or editing timeline.

## Workspace

- `apps/desktop`: Tauri and React desktop shell
- `crates/domain`: shared domain models
- `crates/recorder`: recording contracts and session lifecycle
- `crates/tracker`: mouse tracking contracts
- `crates/focus_engine`: camera timeline generation
- `crates/renderer`: video rendering contracts
- `crates/media`: FFmpeg/FFprobe integration boundary
- `crates/project_store`: project persistence boundary
- `crates/platform_windows`: Windows-specific adapters
- `docs`: architecture and data contracts

## Development status

Phase 6 automatic rendering is implemented for development testing. Each recording produces a source MP4, synchronized mouse trace, deterministic camera timeline, and an automatically rendered final MP4 with eased virtual-camera motion.
