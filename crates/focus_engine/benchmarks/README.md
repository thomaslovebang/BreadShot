# Automatic Director benchmark

This directory documents the versioned interaction benchmark used to develop BreadShot's automatic director.

`tests/automatic_director_benchmark.rs` currently defines 20 deterministic synthetic traces covering idle movement, stable clicks, accidental high-speed clicks, edge targets, repeated intent, conflicting intent, active-target pans, dwell, and timeline integrity.

## What v1 proves

- Known camera invariants are executable and regression-tested.
- Every case has a stable name that can later map to a real captured fixture and rendered reference video.
- Algorithm changes cannot silently break the structural behavior already shipped.

## What v1 does not prove

- Visual quality or user preference.
- End-to-end capture and rendered visual quality for drag, scroll, typing, or UI changes.
- Real capture timing across different Windows hardware.
- The roadmap's 18-of-20 publishability exit gate.

The next benchmark revision will add recorded JSONL traces and expected semantic intent. Rendered MP4 evidence and human review will remain required before Automatic Director 2.0 can pass its release gate.
