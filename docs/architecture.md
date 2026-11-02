# BreadShot architecture

BreadShot is a Windows-first, local-first screen recorder. The desktop shell coordinates capture, privacy-safe interaction tracking, semantic camera direction, cursor rendering, audio mixing, and MP4 export.

## Runtime data flow

1. The React UI selects one display and starts a Tauri recording command.
2. The session coordinator establishes one monotonic clock origin shared by video, audio, and interaction tracking.
3. FFmpeg captures the selected display while Windows adapters capture optional system audio, microphone audio, cursor state, and semantic interaction signals.
4. The focus engine converts the local JSONL interaction trace into a deterministic camera timeline.
5. Cursor assets and audio are composed into the source recording.
6. The renderer applies the camera timeline and atomically publishes the final MP4.
7. On success, processing artifacts are removed so the user-facing folder contains only source and final video.

No account or upload is required for this pipeline.

## Workspace boundaries

```text
apps/desktop                 React UI and Tauri session coordinator
crates/domain                Shared serializable camera and interaction types
crates/platform_windows      Windows tracking, cursor, and capture adapters
crates/focus_engine          Semantic intent and deterministic camera timeline
crates/renderer              FFmpeg camera rendering
crates/project_store         Transactional project and edit metadata
crates/editor_core           Timeline primitives retained for internal compatibility
crates/recorder, tracker     Platform-independent adapter boundaries
```

Dependencies point inward toward `domain`. The focus engine does not depend on Tauri, React, Windows APIs, or FFmpeg process details. Windows-specific unsafe code stays inside `platform_windows`.

## Automatic Director

The tracker records normalized pointer geometry, timing, pointer speed, button transitions, wheel direction, cursor role, and anonymous key activity. It does not record typed characters or key codes.

The focus engine turns those signals into confidence-scored click, double-click, drag, scroll, typing, and dwell candidates. Low-confidence candidates are discarded. Related candidates merge into stable focus sequences before timeline generation.

See [Automatic Director](automatic-director.md) for the current rules and known limitations.

## Reliability invariants

- The source video remains usable if automatic rendering fails.
- Optional enhancements, such as scroll hooks and hardware encoding, degrade without preventing core capture.
- Camera centers are constrained so rendering cannot expose pixels outside the source frame.
- Successful cleanup never removes source or final video.
- User settings and stored project formats remain compatible across internal naming changes.
- The same trace and director profile produce the same camera timeline.

## Verification layers

1. Focused unit tests protect algorithms and serialization.
2. The synthetic v1 benchmark protects 20 deterministic interaction cases.
3. The real v2 benchmark validates privacy-safe captured traces and structural expectations.
4. Human review decides whether at least 18 of 20 rendered outputs are publishable without camera correction.
5. GitHub Actions builds the frontend, prepares verified media resources, tests the Rust workspace, and publishes the current evidence report.
