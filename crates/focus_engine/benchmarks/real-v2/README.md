# Automatic Director real benchmark v2

This benchmark measures whether BreadShot's final camera work is publishable, not merely whether the algorithm returns valid JSON.

## Capture protocol

1. Use a normal Windows desktop at 100% or 125% display scaling. Record one display.
2. Use the `Clean` director profile and the current `main` build.
3. Set `BREADSHOT_BENCHMARK_TRACE_DIR` before launching the development build.
4. Record each task naturally once. Do not slow down or exaggerate movements to help the algorithm.
5. Preserve the final MP4 outside the repository, inspect the exported JSONL, then rename it to the manifest path.
6. Record the commit hash and an evidence URL in the manifest review notes or linked review issue.

Never commit a screen video, filename, window title, typed content, or other private material without explicit permission. The checked-in trace must contain only the tracker fields documented in `traces/README.md`.

## Task scripts

| ID | Reproducible task |
| --- | --- |
| 01 | Open three browser links, switch between two tabs, and return to the first page. |
| 02 | Open a long article, click its reading column, scroll down in three bursts, pause, then scroll up once. |
| 03 | Click three form fields, type non-sensitive sample text, choose a dropdown option, and submit. |
| 04 | In a code editor, open two files, click a function, type a short comment, and switch file. |
| 05 | Open a command palette, type a query, move through results, and choose one command. |
| 06 | In File Explorer, open two nested folders, go back, and select one file. |
| 07 | Double-click one safe test file or folder to open it. |
| 08 | Drag one disposable test file between two visible folders and undo afterward. |
| 09 | Right-click a safe item and select a non-destructive context-menu action. |
| 10 | Toggle three settings and select one dropdown value in a settings screen. |
| 11 | Select spreadsheet cells, edit one sample value, then scroll vertically and horizontally. |
| 12 | Navigate three slides, select one object, move it slightly, then undo. |
| 13 | Activate safe controls near the left, right, top, and bottom edges of one application. |
| 14 | Use four nearby controls in a dense toolbar without pausing artificially. |
| 15 | Alternate between two meaningful targets on opposite sides of the screen four times. |
| 16 | Search quickly across the screen and make one accidental-looking click while still moving. |
| 17 | Click the same working area three times over several seconds while completing one task. |
| 18 | Leave the screen idle, make two non-purposeful pointer moves, and pause again. |
| 19 | Complete one flow containing click, typing, scrolling, and drag actions. |
| 20 | Record a natural 60-second demonstration of a small product feature from start to finish. |

Use disposable local data for every destructive-looking task.

## Human publishability rubric

Review the final MP4 at normal speed. Mark `publishable: true` only when every item passes:

- every zoom has a clear viewer benefit and the intended target is visible;
- no important action is missed because the camera stayed on an obsolete target;
- repeated actions form a stable sequence instead of oscillating or cutting excessively;
- motion feels comfortable and never exposes space outside the source frame;
- the result can be posted without changing camera timing, position, or zoom.

If any item fails, set `publishable: false` and describe the first visible failure with its timestamp. `null` means unreviewed and never counts toward the gate.

## Commands

Development report; missing traces are visible but do not fail the command:

```powershell
cargo run -p focusflow-focus-engine --bin real_benchmark
```

Release gate; requires all 20 structural passes and at least 18 publishable reviews:

```powershell
cargo run -p focusflow-focus-engine --bin real_benchmark -- --require-gate
```
