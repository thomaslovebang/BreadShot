# Phase 4 mouse tracking test

> Phase 9 is the current product stage. This file is a historical test note.

1. Start FocusFlow with `npm run tauri dev`.
2. Record 15–30 seconds while moving slowly, moving quickly, pausing, left-clicking, and right-clicking.
3. Stop the recording.
4. Open the `.mouse.jsonl` path shown in FocusFlow.

Expected:

- The first line has `type: metadata`, schema version, and virtual-desktop bounds.
- Movement lines have increasing monotonic timestamps, pixel and normalized coordinates, and non-negative `speed_px_s`.
- Left and right clicks appear as `left_click` and `right_click` with `click: true`.
- The final mouse timestamp is close to the MP4 duration.
- The trace is stored beside the matching MP4 and remains valid JSON Lines.

