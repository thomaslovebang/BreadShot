# Phase 6 final-video test

## How to test

1. Start the app with `npm run tauri dev`.
2. Record 20–30 seconds.
3. During the recording: move quickly without clicking, pause for at least one second, click a stable point, click a different distant point, then remain idle before stopping.
4. Stop with the configured shortcut or button.
5. Wait while the UI says it is analyzing and rendering. Do not close the app.
6. Open the path labeled `成片`, ending in `.final.mp4`.

## What to inspect

- The final video opens and has approximately the same duration, resolution, and frame rate as the source.
- Fast searching motion does not move the camera.
- A dwell creates a gentle zoom.
- A click creates a stronger zoom centered near the click.
- Distant consecutive targets pan smoothly rather than jumping.
- The frame never reveals black borders.
- The image does not visibly shake while focused.
- The final 700 ms returns to the full frame.
- Text remains acceptably clear at the default maximum zoom of 1.34x.

## What to return

If successful, report whether focus position, smoothness, clarity, and render time feel acceptable.

If anything fails, return:

- the approximate bad timestamp in the final video;
- the source `.mp4` and `.final.mp4` paths;
- the matching `.camera.json`;
- the matching `.render.log`;
- any error shown in the app or terminal.

