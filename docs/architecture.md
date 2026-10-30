# FocusFlow V1 architecture

FocusFlow V1 targets Windows only. Platform-specific code is kept behind adapters so domain and algorithm code remain independent of Tauri and Windows APIs.

## Data flow

1. The desktop UI starts a recording session.
2. The session coordinator establishes one monotonic clock origin.
3. The Windows capture adapter sends screen frames to the media encoder.
4. The Windows input adapter appends timestamped mouse events.
5. The focus engine converts the mouse event stream into a semantic camera timeline.
6. The renderer applies the camera timeline to the source video and atomically publishes `final.mp4`.

## Dependency rule

Dependencies point inward toward `domain`. Algorithm crates do not depend on Tauri, React, FFmpeg process details, or Windows APIs.

```text
desktop -> recorder/tracker/focus_engine/editor_core/renderer/project_store
recorder/tracker -> platform_windows
renderer -> media
editor_core / project_store / focus_engine -> domain
all core crates -> domain
```

## Current stage

Phase 9 is current. The manual workbench edits camera keyframes and re-exports with the existing renderer. Recording still generates the automatic camera timeline and final MP4 without using the workbench. The list below is the historical phase boundary, not a replacement for that pipeline.

## Phase boundaries

- Phase 2: project and module structure
- Phase 3: Windows screen recording
- Phase 4: Windows mouse tracking and clock synchronization
- Phase 5: deterministic focus engine
- Phase 6: FFmpeg renderer
- Phase 7: motion and quality tuning
- Phase 8: in-app preview and clip timeline
- Phase 9: manual camera workbench

