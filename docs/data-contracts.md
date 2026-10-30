# V1 data contracts

All persisted timestamps use integer milliseconds relative to a shared monotonic recording origin. Screen positions are stored both as capture-local pixels and normalized coordinates.

Every persisted document includes a schema version. Mouse events use JSON Lines so an interrupted recording remains recoverable.

The first line is metadata describing the Windows virtual desktop bounds and sampling interval. Remaining lines contain monotonic `time_ms` and fractional `time`, absolute and normalized coordinates, `speed_px_s`, a click flag, and an event name (`move`, `left_click`, or `right_click`). The trace is saved beside the source MP4 as `<recording>.mouse.jsonl`.

Phase 5 writes `<recording>.camera.json`. It contains semantic camera segments with start/end timestamps, from/to poses, easing, and the reason (`dwell`, `click`, `transition`, or `full_frame`). The renderer consumes this file without needing to understand mouse events.
