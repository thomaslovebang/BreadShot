#![doc = "FFmpeg and FFprobe process integration boundary."]

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaTools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl MediaTools {
    #[must_use]
    pub fn new(ffmpeg: impl Into<PathBuf>, ffprobe: impl Into<PathBuf>) -> Self {
        Self {
            ffmpeg: ffmpeg.into(),
            ffprobe: ffprobe.into(),
        }
    }

    #[must_use]
    pub fn is_configured(&self) -> bool {
        Path::new(&self.ffmpeg).is_file() && Path::new(&self.ffprobe).is_file()
    }
}
