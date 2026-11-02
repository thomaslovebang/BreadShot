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

## Real v2 evidence gate

`real-v2/manifest.json` defines 20 real application tasks. Missing traces are reported honestly and do not count as evidence. Run the current report from the repository root:

Follow the [real v2 capture protocol and review rubric](real-v2/README.md) so evidence is reproducible.

```powershell
cargo run -p focusflow-focus-engine --bin real_benchmark
```

Use the strict release gate in CI or before a release candidate:

```powershell
cargo run -p focusflow-focus-engine --bin real_benchmark -- --require-gate
```

To export privacy-safe traces from a development build while preserving the normal two-MP4 output folder, set an external trace directory before launching BreadShot:

```powershell
$env:BREADSHOT_BENCHMARK_TRACE_DIR = "D:\BreadShot-real-traces"
cd apps/desktop
npm run tauri dev
```

Each successful recording copies its internal `.mouse.jsonl` into that developer-only directory before normal cleanup. Inspect and rename the trace to the matching manifest filename. This environment variable is unset in release builds by default and changes nothing for normal users.

Rendered MP4 evidence and explicit human review remain required before Automatic Director 2.0 can pass its 18-of-20 publishability gate.
