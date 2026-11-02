# Contributing to BreadShot

Thanks for helping build a screen recorder whose best edit is no edit.

BreadShot is intentionally focused: a change should improve automatic output quality, recording reliability, time to a publishable result, or trustworthy open-source adoption. Large general-purpose editing features are outside the current product direction.

## High-value contributions

- Reproducible capture, audio, cursor, or export failures on Windows.
- Privacy-safe real interaction traces for the Automatic Director benchmark.
- Small, measured improvements to intent detection and camera stability.
- Compatibility results from Intel, AMD, and NVIDIA Windows machines.
- Clear documentation, translations, and first-run usability fixes.

For substantial behavior or architecture changes, open an issue before investing in implementation. Focused bug fixes and documentation improvements can go directly to a pull request.

## Development setup

Requirements: Windows 10 or 11, Node.js 22+, npm, and Rust 1.85+ with the MSVC toolchain.

```powershell
git clone https://github.com/thomaslovebang/BreadShot.git
cd BreadShot\apps\desktop
npm ci
npm run prepare:ffmpeg
npm run tauri dev
```

`prepare:ffmpeg` downloads the pinned FFmpeg bundle and verifies its SHA-256 checksum. Do not commit generated binaries or legal files.

## Required checks

From the repository root:

```powershell
cargo test --workspace
```

From `apps/desktop`:

```powershell
npm run build
```

For Automatic Director work, also run:

```powershell
cargo run -p focusflow-focus-engine --bin real_benchmark
```

The real benchmark may report missing cases during development. Do not claim its strict release gate passes unless `--require-gate` succeeds with reviewed evidence.

## Automatic Director evidence

Read [the director design](docs/automatic-director.md) and [real v2 protocol](crates/focus_engine/benchmarks/real-v2/README.md) before changing camera rules.

Every algorithm change should include a focused regression test. Changes intended to improve visual quality should also include a privacy-safe real trace and a description of the visible before/after result. Never commit typed text, key codes, filenames, window titles, clipboard content, or private video.

## Pull requests

- Keep one pull request focused on one problem.
- Explain the user-visible behavior and why it matches BreadShot's product rule.
- List the checks you ran and any testing you could not perform.
- Preserve existing user settings and project compatibility unless a migration is included.
- Do not reformat or rename unrelated code.
- Include screenshots or short, privacy-safe videos for visible UI changes.

By contributing, you agree that your contribution is licensed under the repository's MIT License.
