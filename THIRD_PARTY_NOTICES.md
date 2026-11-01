# Third-party notices

FocusFlow's editor architecture and timeline interaction model are informed by
OpenCut and OpenCut Classic. OpenCut is distributed under the MIT License.

- Source: https://github.com/opencut-app/opencut
- Classic source: https://github.com/opencut-app/opencut-classic
- License: MIT

FocusFlow retains attribution for any substantial portions adapted from these
projects. FocusFlow-specific project persistence, camera direction, Windows
recording, mouse tracking, and rendering integration remain independently
implemented in this repository.

## FFmpeg

FocusFlow release installers bundle `ffmpeg.exe` and `ffprobe.exe` from the
Gyan FFmpeg 9.0.2 Essentials build. FFmpeg is a separate executable invoked by
FocusFlow for local screen capture and video rendering. The bundled build is
licensed under GNU GPL version 3.

- Windows build: https://www.gyan.dev/ffmpeg/builds/
- Package: `ffmpeg-9.0.2-essentials_build.zip`
- Package SHA-256: `60f467265b1e312373dbcd92200c2618a74850f98d3d078e94296bb3fa2047ba`
- Corresponding source: https://github.com/FFmpeg/FFmpeg/commit/946fcce07b
- FFmpeg license information: https://ffmpeg.org/legal.html

The installer includes the build's GPLv3 license and build README under its
`legal` resources directory. FocusFlow itself is licensed separately under MIT.
