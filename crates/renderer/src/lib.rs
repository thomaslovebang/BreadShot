#![doc = "Camera timeline video rendering boundary."]

use focusflow_domain::{CameraEasing, CameraSegment, CameraTimeline};
use serde::Deserialize;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub trait Renderer {
    type Error;

    fn render(
        &self,
        source: &Path,
        timeline: &CameraTimeline,
        output: &Path,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone)]
pub struct FfmpegRenderer {
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
}

impl FfmpegRenderer {
    #[must_use]
    pub fn new(ffmpeg: impl Into<PathBuf>, ffprobe: impl Into<PathBuf>) -> Self {
        Self {
            ffmpeg: ffmpeg.into(),
            ffprobe: ffprobe.into(),
        }
    }

    fn probe(&self, source: &Path) -> Result<VideoInfo, String> {
        let output = command(&self.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "v:0",
                "-show_entries",
                "stream=width,height,r_frame_rate",
                "-of",
                "json",
            ])
            .arg(source)
            .output()
            .map_err(|error| format!("无法启动 FFprobe：{error}"))?;
        if !output.status.success() {
            return Err(format!(
                "FFprobe 读取视频失败：{}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let probe: ProbeOutput = serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("无法解析视频信息：{error}"))?;
        let stream = probe
            .streams
            .into_iter()
            .next()
            .ok_or_else(|| "视频中没有画面流".to_string())?;
        let fps = parse_frame_rate(&stream.r_frame_rate)?;
        if stream.width == 0 || stream.height == 0 || fps <= 0.0 {
            return Err("视频尺寸或帧率无效".to_string());
        }
        Ok(VideoInfo {
            width: stream.width,
            height: stream.height,
            fps,
        })
    }
}

impl Renderer for FfmpegRenderer {
    type Error = String;

    fn render(
        &self,
        source: &Path,
        timeline: &CameraTimeline,
        output: &Path,
    ) -> Result<(), Self::Error> {
        let info = self.probe(source)?;
        let filter = build_zoompan_filter(timeline, info);
        let filter_path = output.with_extension("filter.txt");
        fs::write(&filter_path, filter).map_err(|error| format!("无法保存镜头滤镜：{error}"))?;
        let log_path = output.with_extension("render.log");
        let log = File::create(&log_path).map_err(|error| format!("无法创建渲染日志：{error}"))?;
        let log_copy = log
            .try_clone()
            .map_err(|error| format!("无法初始化渲染日志：{error}"))?;

        let status = command(&self.ffmpeg)
            .args(["-hide_banner", "-y", "-i"])
            .arg(source)
            .arg("-/filter:v:0")
            .arg(&filter_path)
            .args([
                "-map",
                "0:v:0",
                "-map",
                "0:a?",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "18",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "copy",
                "-movflags",
                "+faststart",
            ])
            .arg(output)
            .stdin(Stdio::null())
            .stdout(Stdio::from(log_copy))
            .stderr(Stdio::from(log))
            .status()
            .map_err(|error| format!("无法启动视频渲染：{error}"))?;
        if !status.success() {
            return Err(format!("视频渲染失败，请查看 {}", log_path.display()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct VideoInfo {
    width: u32,
    height: u32,
    fps: f64,
}

#[derive(Deserialize)]
struct ProbeOutput {
    streams: Vec<ProbeStream>,
}

#[derive(Deserialize)]
struct ProbeStream {
    width: u32,
    height: u32,
    r_frame_rate: String,
}

fn parse_frame_rate(value: &str) -> Result<f64, String> {
    let (numerator, denominator) = value
        .split_once('/')
        .ok_or_else(|| format!("无效帧率：{value}"))?;
    let numerator: f64 = numerator
        .parse()
        .map_err(|_| format!("无效帧率：{value}"))?;
    let denominator: f64 = denominator
        .parse()
        .map_err(|_| format!("无效帧率：{value}"))?;
    if denominator == 0.0 {
        return Err(format!("无效帧率：{value}"));
    }
    Ok(numerator / denominator)
}

fn build_zoompan_filter(timeline: &CameraTimeline, info: VideoInfo) -> String {
    let zoom = piecewise_expression(&timeline.segments, info.fps, |segment| {
        (segment.from.zoom, segment.to.zoom)
    });
    let center_x = piecewise_expression(&timeline.segments, info.fps, |segment| {
        (segment.from.center.x, segment.to.center.x)
    });
    let center_y = piecewise_expression(&timeline.segments, info.fps, |segment| {
        (segment.from.center.y, segment.to.center.y)
    });
    let x = format!("max(0,min(iw-iw/zoom,iw*({center_x})-iw/zoom/2))");
    let y = format!("max(0,min(ih-ih/zoom,ih*({center_y})-ih/zoom/2))");
    format!(
        "fps={:.6},zoompan=z='{zoom}':x='{x}':y='{y}':d=1:s={}x{}:fps={:.6}",
        info.fps, info.width, info.height, info.fps
    )
}

fn piecewise_expression<F>(segments: &[CameraSegment], fps: f64, values: F) -> String
where
    F: Fn(&CameraSegment) -> (f64, f64),
{
    let fallback = segments
        .last()
        .map_or((1.0, 1.0), |segment| values(segment))
        .1;
    let mut expression = number(fallback);
    for segment in segments.iter().rev() {
        let start = segment.start_ms as f64 * fps / 1_000.0;
        let end = segment.end_ms as f64 * fps / 1_000.0;
        let (from, to) = values(segment);
        let value = if (from - to).abs() < f64::EPSILON || end <= start {
            number(to)
        } else {
            let progress = format!("clip((in-{start:.6})/{:.6},0,1)", end - start);
            let eased = match segment.easing {
                CameraEasing::Linear => progress,
                CameraEasing::EaseInOutCubic => {
                    format!("if(lt({progress},0.5),4*pow({progress},3),1-pow(-2*{progress}+2,3)/2)")
                }
            };
            format!("{}+({})*({eased})", number(from), number(to - from))
        };
        expression = format!("if(between(in,{start:.6},{end:.6}),{value},{expression})");
    }
    expression
}

fn number(value: f64) -> String {
    format!("{value:.9}")
}

fn command(program: &Path) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use focusflow_domain::{CameraPose, FocusReason, NormalizedPoint};

    fn pose(x: f64, y: f64, zoom: f64) -> CameraPose {
        CameraPose {
            center: NormalizedPoint { x, y },
            zoom,
        }
    }

    #[test]
    fn parses_fractional_frame_rate() {
        assert!((parse_frame_rate("60000/1001").unwrap() - 59.940_059).abs() < 0.001);
    }

    #[test]
    fn filter_keeps_source_dimensions_and_one_frame_per_input() {
        let timeline = CameraTimeline {
            schema_version: 1,
            duration_ms: 2_000,
            segments: vec![CameraSegment {
                start_ms: 0,
                end_ms: 2_000,
                from: pose(0.5, 0.5, 1.0),
                to: pose(0.6, 0.4, 1.34),
                easing: CameraEasing::EaseInOutCubic,
                reason: FocusReason::Click,
            }],
        };
        let filter = build_zoompan_filter(
            &timeline,
            VideoInfo {
                width: 1920,
                height: 1080,
                fps: 60.0,
            },
        );
        assert!(filter.contains("d=1:s=1920x1080:fps=60.000000"));
        assert!(filter.starts_with("fps=60.000000,zoompan="));
        assert!(filter.contains("pow("));
        assert!(filter.contains("iw-iw/zoom"));
    }

    #[test]
    fn static_segment_uses_constant_expression() {
        let segment = CameraSegment {
            start_ms: 0,
            end_ms: 1_000,
            from: pose(0.5, 0.5, 1.0),
            to: pose(0.5, 0.5, 1.0),
            easing: CameraEasing::Linear,
            reason: FocusReason::FullFrame,
        };
        let expression =
            piecewise_expression(&[segment], 60.0, |item| (item.from.zoom, item.to.zoom));
        assert!(!expression.contains("pow("));
    }
}
