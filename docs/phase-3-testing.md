# Phase 3 Windows recording test

## Start

From `apps/desktop`, run `npm run tauri dev`.

1. Use **更改** to select an output folder before recording.
2. Click **开始录制** and confirm FocusFlow minimizes.
3. Perform normal desktop activity for 10–30 seconds.
4. Press the configured global shortcut (default: `Ctrl+Shift+F9`).
5. Confirm FocusFlow returns to the foreground after the MP4 is finalized.

## Expected result

- The UI timer advances while recording.
- The output-folder and shortcut controls are disabled while recording.
- The global shortcut starts or stops recording even when another app has focus.
- Starting minimizes FocusFlow; stopping restores and focuses it.
- Stopping changes the status to `原始视频已保存`.
- The exact output path is shown in the UI.
- The MP4 is stored under `%USERPROFILE%\Videos\FocusFlow`.
- The file opens in a normal Windows video player and contains the complete desktop motion at the expected duration.

## Failure evidence

Each MP4 has a matching `.log` file. Keep that log when reporting capture or encoding failures.
