# Real interaction traces

This directory accepts sanitized JSONL traces produced by BreadShot's local tracker. A trace contains normalized pointer coordinates, event timing, button transitions, wheel direction, cursor role, and anonymous key-activity markers. It must not contain video frames, typed text, key codes, filenames, window titles, or clipboard content.

Use the exact filenames declared in `../manifest.json`. Record the matching task naturally, then copy only the JSONL trace here after inspecting it for the allowed fields.

Do not mark a case publishable from the trace alone. Render its corresponding source recording with the same build, review the final MP4 at normal playback speed, and add an evidence URL or reproducible local reference to the manifest review entry.

The release gate is intentionally strict:

- all 20 real traces must exist and pass structural expectations;
- at least 18 final videos must be explicitly reviewed as publishable without camera correction;
- missing traces and `publishable: null` never count as success.
