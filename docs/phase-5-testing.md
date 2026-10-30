# Phase 5 Focus Engine test

> Phase 9 is the current product stage. This file is a historical test note.

Record 20–30 seconds containing these behaviors:

1. Move the pointer quickly without clicking.
2. Stop on one area for at least one second.
3. Left-click a stable position.
4. Click two distant positions in quick succession.
5. Leave the pointer idle for several seconds.

After stopping, open the `.camera.json` path shown in FocusFlow.

Expected:

- Rapid searching motion does not create focus segments.
- Dwell creates a `dwell` segment around zoom `1.24`.
- Click creates a `click` segment around zoom `1.34`.
- Nearby cues merge instead of repeatedly zooming.
- Distant consecutive clicks produce an eased focus-to-focus transition.
- Long idle gaps return the camera to zoom `1.0`.
- All crop centers are constrained so Phase 6 cannot reveal black borders.

