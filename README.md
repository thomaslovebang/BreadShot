# BreadShot

> The zero-edit screen recorder for Windows.

[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011-2563eb?logo=windows)](https://github.com/thomaslovebang/BreadShot/releases/latest)
[![Release](https://img.shields.io/github/v/release/thomaslovebang/BreadShot?include_prereleases)](https://github.com/thomaslovebang/BreadShot/releases/latest)
[![License](https://img.shields.io/badge/license-MIT-22c55e)](LICENSE)
[![Local first](https://img.shields.io/badge/processing-local--first-111827)](#private-by-default)

[中文说明](README.zh-CN.md) · [Download](https://github.com/thomaslovebang/BreadShot/releases/latest) · [Report a bug](https://github.com/thomaslovebang/BreadShot/issues/new?template=bug_report.yml)

BreadShot records your screen, follows the action, and turns the result into a polished MP4 automatically. There is no timeline to learn, no account to create, no watermark, and no upload before you can export.

Press one key. Show your work. Get the shot.

> BreadShot was previously called FocusFlow. The project was renamed before its first major public launch so it could grow under a distinct, long-term identity.

## Why BreadShot

Most screen recorders give you raw footage. Video editors give you another job.

BreadShot is built around a different workflow:

1. Choose a display and press your global shortcut.
2. Demonstrate your product, tutorial, or bug naturally.
3. Press the shortcut again.
4. BreadShot analyzes the interaction, directs the camera, and exports the final MP4 locally.

The goal is not to become a general-purpose editor. The goal is to make editing unnecessary for everyday screen videos.

## What works today

- Unlimited recording duration with no watermark.
- One-display capture on multi-monitor Windows systems.
- Automatic zoom and pan generated from mouse activity.
- Native system-audio capture through WASAPI loopback.
- Optional microphone capture, synchronized and mixed into the MP4.
- Custom global start/stop shortcut; `F8` works immediately by default.
- Per-role `.cur` and animated `.ani` cursor replacement for all 17 Windows cursor roles.
- Global cursor sizing based on the real Windows cursor baseline.
- Persistent display, folder, shortcut, audio, language, and cursor settings.
- Local processing with NVIDIA NVENC acceleration when available.
- A clean output folder containing only the source and automatically directed MP4.
- Chinese, English, Japanese, French, German, Spanish, and Arabic interfaces.

## Private by default

Screen frames, mouse activity, microphone audio, and custom cursor files stay on your computer. BreadShot has no account system and does not require a cloud service to record or render.

## Product direction

BreadShot is becoming an **automatic director**, not a manual timeline editor.

The next major milestone is Automatic Director 2.0:

- Understand clicks, double-clicks, dragging, scrolling, typing, and dwell time as different intentions.
- Combine related actions into stable camera sequences instead of reacting to every event.
- Prefer no zoom over a wrong zoom when confidence is low.
- Add reliable window and region capture.
- Automatically compose backgrounds, padding, corners, and output aspect ratios.
- Make ten consecutive recordings feel boringly reliable.

See the public issues for implementation-sized tasks as this roadmap is broken down.

## Download

Download the latest Windows installer from [GitHub Releases](https://github.com/thomaslovebang/BreadShot/releases/latest).

The installer bundles the video engine. You do not need to install FFmpeg, Node.js, or Rust. BreadShot currently targets 64-bit Windows 10 and Windows 11. Public alpha installers are not code-signed yet, so Windows SmartScreen may ask for confirmation.

## Build from source

Requirements: Windows 10/11, Node.js 22+, the Rust toolchain, and npm.

```powershell
cd apps/desktop
npm install
npm run prepare:ffmpeg
npm run tauri dev
```

Check the Rust workspace from the repository root:

```powershell
cargo check --workspace
```

Create the NSIS installer:

```powershell
cd apps/desktop
npm run bundle
```

`prepare:ffmpeg` downloads the pinned FFmpeg package and verifies its SHA-256 checksum before preparing the bundled executables and legal files.

## Repository map

- `apps/desktop` — Tauri and React desktop application.
- `crates/focus_engine` — automatic camera timeline generation.
- `crates/platform_windows` — Windows capture, tracking, cursor, and audio adapters.
- `crates/renderer` — FFmpeg rendering pipeline.
- `crates/domain` — shared domain models.
- `docs` — architecture and historical engineering notes.

Some internal crate and storage identifiers still use the former `focusflow` name to preserve upgrade compatibility. They are implementation details and will be migrated only when that can be done without losing user settings.

## Contributing

BreadShot is early, opinionated, and open source. The most valuable contributions are reproducible recording failures, difficult real-world interaction samples, Windows hardware compatibility reports, and focused improvements to the automatic director.

Before opening a feature request, keep the product rule in mind: **record once, edit less, publish faster**.

## License

BreadShot is available under the [MIT License](LICENSE). Release installers bundle separate FFmpeg executables under GPLv3; see [third-party notices](THIRD_PARTY_NOTICES.md).
