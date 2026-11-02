# Automatic Director

BreadShot's Automatic Director converts a local interaction trace into a deterministic camera timeline. Its product rule is simple: **prefer no zoom over a wrong zoom**.

This document describes the behavior implemented on `main`. It is intentionally concrete so algorithm changes can be reviewed and benchmarked instead of judged only by feel.

## Privacy boundary

The Windows adapter records timing, normalized pointer position, pointer speed, button transitions, wheel direction, cursor role, and an anonymous keyboard-activity signal.

It does not record key codes, typed characters, clipboard content, window text, or page content. Keyboard activity means only that at least one key transitioned to pressed during a sampled interval. Scroll capture records direction, not the document being viewed.

The interaction trace is used locally while producing the video. Successful user-facing output remains the source MP4 and directed MP4.

## Intent candidates

The engine currently produces these candidates:

| Intent | Evidence | Camera behavior |
| --- | --- | --- |
| Stable click | A click after the pointer has settled | Focus the clicked area |
| Double click | Two same-button clicks within 500 ms and a small radius | Produce one stronger focus instead of two cuts |
| Drag | A left-button press/release pair with meaningful travel | Pan from drag start to drag end |
| Typing | Anonymous key activity after a recent click | Keep the clicked area focused; never invent a target |
| Scroll | Wheel activity after a recent click | Keep the clicked area focused; never chase the scrollbar |
| Dwell | A sufficiently long, spatially stable pause | Apply a gentle focus |

Fast search movement, unanchored typing, unanchored scrolling, and spatially unstable pauses do not create a camera target.

## Confidence and profiles

Every candidate receives an internal confidence score. Double clicks and drags carry strong evidence. Click confidence decreases with pointer speed. Dwell confidence combines duration and spatial stability. Typing and scrolling are accepted only when a recent click supplies an anchor.

The selected director profile defines a minimum confidence threshold:

- **Dynamic** accepts more candidates and moves faster.
- **Clean** is the balanced default.
- **Soft** is the most conservative and uses slower, smaller movements.

Candidates below the profile threshold are discarded before timeline construction. This makes the no-zoom fallback an explicit algorithm rule rather than an accident of missing data.

## Stable timeline construction

Accepted candidates are merged when they refer to the same nearby target and overlap in time. Repeated activity extends a focus hold instead of creating cuts. Distant active targets receive a bounded focus-to-focus pan. Every center is constrained so the camera cannot expose space outside the source frame.

The generated timeline is deterministic: the same trace and profile always produce the same segments.

## Verification

The engine has focused unit tests for false-positive suppression, double-click collapsing, drag pans, typing and scrolling anchors, confidence thresholds, profile differences, edge constraints, and timeline restoration.

The versioned v1 benchmark adds 20 deterministic interaction scenarios. It protects structural behavior, but it is not yet proof of visual quality. The next evidence layer is a checked-in set of real JSONL traces, rendered reference clips, and human publishability review.

## Current limits

- Confidence is heuristic rather than learned from labeled real-world traces.
- The engine does not yet observe window bounds, controls, text caret position, or post-click UI changes.
- A recent click is required before typing or scrolling can extend a focus.
- Visual quality still needs real recorded fixtures across different display scales and applications.

These limits are deliberate. BreadShot will expand semantic input only when it can preserve the local-first privacy boundary and measurably reduce wrong camera moves.
