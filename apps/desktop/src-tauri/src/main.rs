#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use focusflow_domain::CameraTimeline;
use focusflow_editor_core::{
    CameraKeyframe, Timeline as EditorTimeline, VideoClip, timeline_from_keyframes,
};
use focusflow_focus_engine::{DirectorProfile, SmartFocusEngine};
use focusflow_platform_windows::{CaptureBounds, MouseTrackerHandle};
use focusflow_project_store::{
    EditOperation, EditTimeline, FocusFlowProject, PROJECT_SCHEMA_VERSION, ProjectAssets,
    ProjectState, VideoClipEdit, load_edit_timeline, load_project, save_edit_timeline,
    save_project,
};
use focusflow_renderer::{FfmpegRenderer, Renderer};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, State};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    #[cfg(target_os = "windows")]
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

struct ActiveRecording {
    process: Child,
    output_path: PathBuf,
    mouse_events_path: PathBuf,
    camera_timeline_path: PathBuf,
    final_video_path: PathBuf,
    project_path: PathBuf,
    project: FocusFlowProject,
    ffmpeg_path: PathBuf,
    ffprobe_path: PathBuf,
    director_profile: DirectorProfile,
    mouse_tracker: Option<MouseTrackerHandle>,
}

struct RecorderState {
    active: Mutex<Option<ActiveRecording>>,
    output_directory: Mutex<PathBuf>,
    director_profile: Mutex<DirectorProfile>,
    last_final_video: Mutex<Option<PathBuf>>,
    last_source_video: Mutex<Option<PathBuf>>,
}

impl RecorderState {
    fn new() -> Self {
        Self {
            active: Mutex::new(None),
            output_directory: Mutex::new(default_recordings_dir()),
            director_profile: Mutex::new(DirectorProfile::Clean),
            last_final_video: Mutex::new(None),
            last_source_video: Mutex::new(None),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessingProgress {
    stage: &'static str,
    progress: u8,
    message: &'static str,
}

fn emit_processing_progress(
    app: &tauri::AppHandle,
    stage: &'static str,
    progress: u8,
    message: &'static str,
) {
    let _ = app.emit(
        "processing-progress",
        ProcessingProgress {
            stage,
            progress,
            message,
        },
    );
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecorderStatus {
    is_recording: bool,
    ffmpeg_available: bool,
    output_path: Option<String>,
    output_directory: String,
    mouse_events_path: Option<String>,
    camera_timeline_path: Option<String>,
    final_video_path: Option<String>,
    project_path: Option<String>,
    director_profile: DirectorProfile,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CaptureArea {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioProject {
    project_path: String,
    title: String,
    final_video_path: String,
    duration_ms: u64,
    camera_timeline: CameraTimeline,
    /// Automatic camera path before manual keyframe edits, in the same timebase as `camera_timeline`.
    baseline_camera_timeline: CameraTimeline,
    source_video_path: String,
    video_clips: Vec<VideoClipEdit>,
    camera_segment_source_indices: Vec<usize>,
    preview_revision: u64,
}

fn bundled_ffmpeg(app: &tauri::AppHandle) -> Option<PathBuf> {
    let resource = app
        .path()
        .resource_dir()
        .ok()?
        .join("bin")
        .join("ffmpeg.exe");
    resource.is_file().then_some(resource)
}

fn development_ffmpeg() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("bin")
        .join("ffmpeg.exe");
    path.is_file().then_some(path)
}

fn winget_ffmpeg() -> Option<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from)?;
    let link = local_app_data
        .join("Microsoft")
        .join("WinGet")
        .join("Links")
        .join("ffmpeg.exe");
    if link.is_file() {
        return Some(link);
    }

    let package = local_app_data
        .join("Microsoft")
        .join("WinGet")
        .join("Packages")
        .join("Gyan.FFmpeg.Essentials_Microsoft.Winget.Source_8wekyb3d8bbwe");
    for entry in fs::read_dir(package).ok()?.flatten() {
        let candidate = entry.path().join("bin").join("ffmpeg.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn ffmpeg_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    if let Some(path) = bundled_ffmpeg(app) {
        return Some(path);
    }
    if let Some(path) = development_ffmpeg() {
        return Some(path);
    }
    if let Some(path) = winget_ffmpeg() {
        return Some(path);
    }

    let status = command("ffmpeg")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    matches!(status, Ok(result) if result.success()).then(|| PathBuf::from("ffmpeg"))
}

fn matching_ffprobe(ffmpeg: &PathBuf) -> PathBuf {
    if ffmpeg == &PathBuf::from("ffmpeg") {
        return PathBuf::from("ffprobe");
    }
    let adjacent = ffmpeg.with_file_name("ffprobe.exe");
    if adjacent.is_file() {
        adjacent
    } else {
        PathBuf::from("ffprobe")
    }
}

fn default_recordings_dir() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Videos")
        .join("FocusFlow")
}

fn new_output_path(directory: &PathBuf) -> Result<PathBuf, String> {
    fs::create_dir_all(directory).map_err(|error| format!("无法创建录制目录：{error}"))?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("系统时间异常：{error}"))?
        .as_secs();
    Ok(directory.join(format!("recording-{timestamp}.mp4")))
}

#[tauri::command]
fn recorder_status(app: tauri::AppHandle, state: State<'_, RecorderState>) -> RecorderStatus {
    let active = state
        .active
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let output_directory = state
        .output_directory
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let director_profile = *state
        .director_profile
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let last_final_video = state
        .last_final_video
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    RecorderStatus {
        is_recording: active.is_some(),
        ffmpeg_available: ffmpeg_path(&app).is_some(),
        output_path: active
            .as_ref()
            .map(|recording| recording.output_path.display().to_string()),
        output_directory: output_directory.display().to_string(),
        mouse_events_path: active
            .as_ref()
            .map(|recording| recording.mouse_events_path.display().to_string()),
        camera_timeline_path: active
            .as_ref()
            .map(|recording| recording.camera_timeline_path.display().to_string()),
        final_video_path: active
            .as_ref()
            .map(|recording| recording.final_video_path.display().to_string())
            .or_else(|| last_final_video.map(|path| path.display().to_string())),
        project_path: active
            .as_ref()
            .map(|recording| recording.project_path.display().to_string()),
        director_profile,
    }
}

#[tauri::command]
fn set_output_directory(
    directory: String,
    state: State<'_, RecorderState>,
) -> Result<RecorderStatus, String> {
    let active = state
        .active
        .lock()
        .map_err(|_| "录制状态不可用".to_string())?;
    if active.is_some() {
        return Err("录制过程中不能更改保存位置".to_string());
    }

    let path = PathBuf::from(directory.trim());
    if path.as_os_str().is_empty() {
        return Err("保存位置不能为空".to_string());
    }
    fs::create_dir_all(&path).map_err(|error| format!("无法使用该保存位置：{error}"))?;
    if !path.is_dir() {
        return Err("选择的位置不是文件夹".to_string());
    }

    let mut output_directory = state
        .output_directory
        .lock()
        .map_err(|_| "保存位置状态不可用".to_string())?;
    *output_directory = path;
    let director_profile = *state
        .director_profile
        .lock()
        .map_err(|_| "导演风格状态不可用".to_string())?;
    let last_final_video = state
        .last_final_video
        .lock()
        .map_err(|_| "最近视频状态不可用".to_string())?
        .as_ref()
        .map(|path| path.display().to_string());
    Ok(RecorderStatus {
        is_recording: false,
        ffmpeg_available: true,
        output_path: None,
        output_directory: output_directory.display().to_string(),
        mouse_events_path: None,
        camera_timeline_path: None,
        final_video_path: last_final_video,
        project_path: None,
        director_profile,
    })
}

#[tauri::command]
fn set_director_profile(
    profile: DirectorProfile,
    state: State<'_, RecorderState>,
) -> Result<DirectorProfile, String> {
    let active = state
        .active
        .lock()
        .map_err(|_| "录制状态不可用".to_string())?;
    if active.is_some() {
        return Err("录制过程中不能更改导演风格".to_string());
    }
    let mut director_profile = state
        .director_profile
        .lock()
        .map_err(|_| "导演风格状态不可用".to_string())?;
    *director_profile = profile;
    Ok(profile)
}

#[tauri::command]
fn open_project(
    app: tauri::AppHandle,
    path: String,
    state: State<'_, RecorderState>,
) -> Result<RecorderStatus, String> {
    let active = state
        .active
        .lock()
        .map_err(|_| "录制状态不可用".to_string())?;
    if active.is_some() {
        return Err("录制过程中不能打开项目".to_string());
    }
    let project_path = PathBuf::from(path);
    let project = load_project(&project_path)?;
    let parent = project_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let resolve = |asset: &PathBuf| parent.join(asset);
    let director_profile = match project.director_profile.as_str() {
        "clean" => DirectorProfile::Clean,
        "dynamic" => DirectorProfile::Dynamic,
        "soft" => DirectorProfile::Soft,
        _ => return Err("项目包含未知的导演风格".to_string()),
    };
    *state
        .director_profile
        .lock()
        .map_err(|_| "导演风格状态不可用".to_string())? = director_profile;
    let output_directory = state
        .output_directory
        .lock()
        .map_err(|_| "保存位置状态不可用".to_string())?
        .display()
        .to_string();
    Ok(RecorderStatus {
        is_recording: false,
        ffmpeg_available: ffmpeg_path(&app).is_some(),
        output_path: Some(resolve(&project.assets.source_video).display().to_string()),
        output_directory,
        mouse_events_path: Some(resolve(&project.assets.mouse_trace).display().to_string()),
        camera_timeline_path: Some(
            resolve(&project.assets.camera_timeline)
                .display()
                .to_string(),
        ),
        final_video_path: Some(resolve(&project.assets.final_video).display().to_string()),
        project_path: Some(project_path.display().to_string()),
        director_profile,
    })
}

#[tauri::command]
fn load_studio_project(app: tauri::AppHandle, path: String) -> Result<StudioProject, String> {
    let project_path = PathBuf::from(path);
    let project = load_project(&project_path)?;
    let parent = project_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let final_video_path = parent.join(&project.assets.final_video);
    if !final_video_path.is_file() {
        return Err(format!(
            "项目最终视频不存在：{}",
            final_video_path.display()
        ));
    }
    let preview_revision = final_video_path
        .metadata()
        .and_then(|metadata| metadata.modified())
        .and_then(|modified| {
            modified
                .duration_since(UNIX_EPOCH)
                .map_err(std::io::Error::other)
        })
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .map_err(|error| format!("无法读取成片版本：{error}"))?;
    let preview_stem = final_video_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法生成预览文件名".to_string())?;
    let preview_path =
        final_video_path.with_file_name(format!("{preview_stem}.preview-{preview_revision}.mp4"));
    if !preview_path.is_file() {
        fs::copy(&final_video_path, &preview_path)
            .map_err(|error| format!("无法创建版本化预览：{error}"))?;
    }
    let camera_path = parent.join(&project.assets.camera_timeline);
    let camera_file =
        File::open(&camera_path).map_err(|error| format!("无法读取项目镜头时间线：{error}"))?;
    let automatic_timeline: CameraTimeline =
        serde_json::from_reader(BufReader::new(camera_file))
            .map_err(|error| format!("项目镜头时间线无效：{error}"))?;
    let edit_path = parent.join(&project.assets.edit_timeline);
    let edits = load_edit_timeline(&edit_path)?;
    let video_clips = edits
        .operations
        .iter()
        .rev()
        .find_map(|operation| match operation {
            EditOperation::SetVideoClips { clips } => Some(clips.clone()),
            _ => None,
        })
        .unwrap_or_else(|| {
            vec![VideoClipEdit {
                id: "clip-1".to_string(),
                source_start_ms: 0,
                source_end_ms: automatic_timeline.duration_ms,
            }]
        });
    let has_video_edits = edits
        .operations
        .iter()
        .any(|operation| matches!(operation, EditOperation::SetVideoClips { .. }));
    let baseline_camera_timeline = if has_video_edits {
        remap_camera_timeline(&automatic_timeline, &video_clips)
    } else {
        automatic_timeline.clone()
    };
    let (camera_timeline, camera_segment_source_indices) =
        if let Some(manual) = edits.replaced_camera_timeline() {
            let timeline = manual.clone();
            let indices = (0..timeline.segments.len()).collect();
            (timeline, indices)
        } else {
            let mut timeline = automatic_timeline;
            apply_camera_overrides(&mut timeline, &edits)?;
            let indices = if has_video_edits {
                camera_source_indices(&timeline, &video_clips)
            } else {
                (0..timeline.segments.len()).collect()
            };
            if has_video_edits {
                timeline = remap_camera_timeline(&timeline, &video_clips);
            }
            (timeline, indices)
        };
    let source_video = parent.join(&project.assets.source_video);
    let source_video_path = if source_video.is_file() {
        app.asset_protocol_scope()
            .allow_file(&source_video)
            .map_err(|error| format!("无法授权原始视频预览：{error}"))?;
        source_video.display().to_string()
    } else {
        String::new()
    };
    app.asset_protocol_scope()
        .allow_file(&preview_path)
        .map_err(|error| format!("无法授权本地视频预览：{error}"))?;
    Ok(StudioProject {
        project_path: project_path.display().to_string(),
        title: project.title,
        final_video_path: preview_path.display().to_string(),
        duration_ms: camera_timeline.duration_ms,
        camera_timeline,
        baseline_camera_timeline,
        source_video_path,
        video_clips,
        camera_segment_source_indices,
        preview_revision,
    })
}

fn apply_camera_overrides(
    timeline: &mut CameraTimeline,
    edits: &EditTimeline,
) -> Result<(), String> {
    for operation in &edits.operations {
        if let EditOperation::CameraOverride {
            segment_index,
            center_x,
            center_y,
            zoom,
        } = operation
        {
            if !(center_x.is_finite()
                && center_y.is_finite()
                && zoom.is_finite()
                && (0.0..=1.0).contains(center_x)
                && (0.0..=1.0).contains(center_y)
                && (1.0..=3.0).contains(zoom))
            {
                return Err("镜头覆盖参数超出安全范围".to_string());
            }
            let pose = {
                let segment = timeline
                    .segments
                    .get_mut(*segment_index)
                    .ok_or_else(|| "镜头覆盖指向不存在的区段".to_string())?;
                let was_static = segment.from == segment.to;
                segment.to.center.x = *center_x;
                segment.to.center.y = *center_y;
                segment.to.zoom = *zoom;
                if was_static {
                    segment.from = segment.to;
                }
                segment.to
            };
            if let Some(next) = timeline.segments.get_mut(segment_index + 1) {
                next.from = pose;
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn update_camera_segment(
    app: tauri::AppHandle,
    project_path: String,
    segment_index: usize,
    center_x: f64,
    center_y: f64,
    zoom: f64,
) -> Result<StudioProject, String> {
    if !(center_x.is_finite()
        && center_y.is_finite()
        && zoom.is_finite()
        && (0.0..=1.0).contains(&center_x)
        && (0.0..=1.0).contains(&center_y)
        && (1.0..=3.0).contains(&zoom))
    {
        return Err("镜头参数无效".to_string());
    }
    let project_path = PathBuf::from(project_path);
    let ffmpeg = ffmpeg_path(&app).ok_or_else(|| "未找到 FFmpeg".to_string())?;
    let ffprobe = matching_ffprobe(&ffmpeg);
    let render_project_path = project_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = load_project(&render_project_path)?;
        let parent = render_project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let edit_path = parent.join(&project.assets.edit_timeline);
        let mut edits = load_edit_timeline(&edit_path)?;
        edits.operations.retain(|operation| {
            !matches!(operation, EditOperation::CameraOverride { segment_index: existing, .. } if *existing == segment_index)
        });
        edits.operations.push(EditOperation::CameraOverride {
            segment_index,
            center_x,
            center_y,
            zoom,
        });
        render_project_edits(&ffmpeg, &ffprobe, &render_project_path, &project, &edits)?;
        save_edit_timeline(&edit_path, &edits)
    })
    .await
    .map_err(|error| format!("后台重新渲染任务失败：{error}"))??;
    load_studio_project(app, project_path.display().to_string())
}

#[tauri::command]
async fn update_video_clips(
    app: tauri::AppHandle,
    project_path: String,
    clips: Vec<VideoClipEdit>,
) -> Result<StudioProject, String> {
    let editor_timeline = EditorTimeline {
        clips: clips
            .iter()
            .map(|clip| VideoClip {
                id: clip.id.clone(),
                source_start_ms: clip.source_start_ms,
                source_end_ms: clip.source_end_ms,
            })
            .collect(),
    };
    editor_timeline.validate()?;
    let project_path = PathBuf::from(project_path);
    let ffmpeg = ffmpeg_path(&app).ok_or_else(|| "未找到 FFmpeg".to_string())?;
    let ffprobe = matching_ffprobe(&ffmpeg);
    let render_project_path = project_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = load_project(&render_project_path)?;
        let parent = render_project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let edit_path = parent.join(&project.assets.edit_timeline);
        let mut edits = load_edit_timeline(&edit_path)?;
        edits.operations.retain(|operation| {
            !matches!(
                operation,
                EditOperation::SetVideoClips { .. } | EditOperation::SetCameraTimeline { .. }
            )
        });
        edits
            .operations
            .push(EditOperation::SetVideoClips { clips });
        render_project_edits(&ffmpeg, &ffprobe, &render_project_path, &project, &edits)?;
        save_edit_timeline(&edit_path, &edits)
    })
    .await
    .map_err(|error| format!("后台时间线渲染任务失败：{error}"))??;
    load_studio_project(app, project_path.display().to_string())
}

fn render_project_edits(
    ffmpeg: &PathBuf,
    ffprobe: &PathBuf,
    project_path: &std::path::Path,
    project: &FocusFlowProject,
    edits: &EditTimeline,
) -> Result<(), String> {
    let parent = project_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let camera_file = File::open(parent.join(&project.assets.camera_timeline))
        .map_err(|error| format!("无法读取原始镜头时间线：{error}"))?;
    let mut timeline: CameraTimeline = serde_json::from_reader(BufReader::new(camera_file))
        .map_err(|error| format!("原始镜头时间线无效：{error}"))?;
    let manual = edits.replaced_camera_timeline().cloned();
    let clips = edits
        .operations
        .iter()
        .rev()
        .find_map(|operation| match operation {
            EditOperation::SetVideoClips { clips } => Some(clips.as_slice()),
            _ => None,
        });
    let source = parent.join(&project.assets.source_video);
    let final_video = parent.join(&project.assets.final_video);
    if let Some(clips) = clips {
        let edited_source = project_path.with_extension("edit-source.mp4");
        render_clip_sequence(ffmpeg, &source, &edited_source, clips)?;
        let edited_timeline = if let Some(manual) = manual {
            manual
        } else {
            apply_camera_overrides(&mut timeline, edits)?;
            remap_camera_timeline(&timeline, clips)
        };
        FfmpegRenderer::new(ffmpeg, ffprobe).render(&edited_source, &edited_timeline, &final_video)
    } else if let Some(manual) = manual {
        FfmpegRenderer::new(ffmpeg, ffprobe).render(&source, &manual, &final_video)
    } else {
        apply_camera_overrides(&mut timeline, edits)?;
        FfmpegRenderer::new(ffmpeg, ffprobe).render(&source, &timeline, &final_video)
    }
}

#[tauri::command]
async fn save_studio_edits(
    app: tauri::AppHandle,
    project_path: String,
    keyframes: Vec<CameraKeyframe>,
) -> Result<StudioProject, String> {
    let timeline = timeline_from_keyframes(&keyframes)?;
    let project_path = PathBuf::from(project_path);
    let ffmpeg = ffmpeg_path(&app).ok_or_else(|| "未找到 FFmpeg".to_string())?;
    let ffprobe = matching_ffprobe(&ffmpeg);
    let render_project_path = project_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = load_project(&render_project_path)?;
        let parent = render_project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let edit_path = parent.join(&project.assets.edit_timeline);
        let mut edits = load_edit_timeline(&edit_path)?;
        edits.operations.retain(|operation| {
            !matches!(
                operation,
                EditOperation::SetCameraTimeline { .. } | EditOperation::CameraOverride { .. }
            )
        });
        edits
            .operations
            .push(EditOperation::SetCameraTimeline { timeline });
        render_project_edits(&ffmpeg, &ffprobe, &render_project_path, &project, &edits)?;
        save_edit_timeline(&edit_path, &edits)
    })
    .await
    .map_err(|error| format!("后台重新渲染任务失败：{error}"))??;
    load_studio_project(app, project_path.display().to_string())
}

#[tauri::command]
async fn reset_camera_edits(
    app: tauri::AppHandle,
    project_path: String,
) -> Result<StudioProject, String> {
    let project_path = PathBuf::from(project_path);
    let ffmpeg = ffmpeg_path(&app).ok_or_else(|| "未找到 FFmpeg".to_string())?;
    let ffprobe = matching_ffprobe(&ffmpeg);
    let render_project_path = project_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = load_project(&render_project_path)?;
        let parent = render_project_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let edit_path = parent.join(&project.assets.edit_timeline);
        let mut edits = load_edit_timeline(&edit_path)?;
        edits.operations.retain(|operation| {
            !matches!(
                operation,
                EditOperation::SetCameraTimeline { .. } | EditOperation::CameraOverride { .. }
            )
        });
        render_project_edits(&ffmpeg, &ffprobe, &render_project_path, &project, &edits)?;
        save_edit_timeline(&edit_path, &edits)
    })
    .await
    .map_err(|error| format!("后台重新渲染任务失败：{error}"))??;
    load_studio_project(app, project_path.display().to_string())
}

fn render_clip_sequence(
    ffmpeg: &PathBuf,
    source: &std::path::Path,
    output: &std::path::Path,
    clips: &[VideoClipEdit],
) -> Result<(), String> {
    let mut filters = Vec::with_capacity(clips.len() + 1);
    for (index, clip) in clips.iter().enumerate() {
        filters.push(format!(
            "[0:v]trim=start={:.3}:end={:.3},setpts=PTS-STARTPTS[v{index}]",
            clip.source_start_ms as f64 / 1000.0,
            clip.source_end_ms as f64 / 1000.0
        ));
    }
    let inputs = (0..clips.len())
        .map(|index| format!("[v{index}]"))
        .collect::<String>();
    filters.push(format!("{inputs}concat=n={}:v=1:a=0[outv]", clips.len()));
    let status = command(ffmpeg)
        .args(["-hide_banner", "-y", "-i"])
        .arg(source)
        .args(["-filter_complex", &filters.join(";"), "-map", "[outv]"])
        .args([
            "-c:v", "libx264", "-preset", "veryfast", "-crf", "18", "-pix_fmt", "yuv420p",
        ])
        .arg(output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("无法启动片段渲染：{error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| "片段渲染失败".to_string())
}

fn remap_camera_timeline(original: &CameraTimeline, clips: &[VideoClipEdit]) -> CameraTimeline {
    let mut output_offset = 0_u64;
    let mut segments: Vec<focusflow_domain::CameraSegment> = Vec::new();
    for clip in clips {
        for segment in &original.segments {
            let start = segment.start_ms.max(clip.source_start_ms);
            let end = segment.end_ms.min(clip.source_end_ms);
            if start >= end {
                continue;
            }
            let mut mapped = segment.clone();
            mapped.start_ms = output_offset + start - clip.source_start_ms;
            mapped.end_ms = output_offset + end - clip.source_start_ms;
            if let Some(previous) = segments.last() {
                mapped.from = previous.to;
            }
            segments.push(mapped);
        }
        output_offset += clip.source_end_ms - clip.source_start_ms;
    }
    CameraTimeline {
        schema_version: original.schema_version,
        duration_ms: output_offset,
        segments,
    }
}

fn camera_source_indices(original: &CameraTimeline, clips: &[VideoClipEdit]) -> Vec<usize> {
    clips
        .iter()
        .flat_map(|clip| {
            original
                .segments
                .iter()
                .enumerate()
                .filter_map(move |(index, segment)| {
                    (segment.start_ms.max(clip.source_start_ms)
                        < segment.end_ms.min(clip.source_end_ms))
                    .then_some(index)
                })
        })
        .collect()
}

#[tauri::command]
fn start_recording(
    capture_area: CaptureArea,
    app: tauri::AppHandle,
    state: State<'_, RecorderState>,
) -> Result<RecorderStatus, String> {
    let mut active = state
        .active
        .lock()
        .map_err(|_| "录制状态不可用".to_string())?;
    if active.is_some() {
        return Err("录制已经开始".to_string());
    }

    let capture_width = capture_area.width - capture_area.width.rem_euclid(2);
    let capture_height = capture_area.height - capture_area.height.rem_euclid(2);
    let capture_bounds = CaptureBounds::new(
        capture_area.x,
        capture_area.y,
        capture_width,
        capture_height,
    )?;

    let ffmpeg = ffmpeg_path(&app).ok_or_else(|| "未找到 FFmpeg，暂时无法开始录制".to_string())?;
    let ffprobe = matching_ffprobe(&ffmpeg);
    let output_directory = state
        .output_directory
        .lock()
        .map_err(|_| "保存位置状态不可用".to_string())?
        .clone();
    let director_profile = *state
        .director_profile
        .lock()
        .map_err(|_| "导演风格状态不可用".to_string())?;
    let output_path = new_output_path(&output_directory)?;
    let mouse_events_path = output_path.with_extension("mouse.jsonl");
    let camera_timeline_path = output_path.with_extension("camera.json");
    let final_video_path = output_path.with_extension("final.mp4");
    let edit_timeline_path = output_path.with_extension("edit.json");
    let project_path = output_path.with_extension("focusflow");
    let project_id = output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法生成项目名称".to_string())?
        .to_string();
    let created_at_unix_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("系统时间异常：{error}"))?
        .as_millis();
    let asset_name = |path: &PathBuf| {
        path.file_name()
            .map(PathBuf::from)
            .ok_or_else(|| "无法生成项目素材路径".to_string())
    };
    let project = FocusFlowProject {
        schema_version: PROJECT_SCHEMA_VERSION,
        project_id: project_id.clone(),
        title: project_id,
        created_at_unix_ms,
        platform: "windows".to_string(),
        state: ProjectState::Recording,
        duration_ms: None,
        director_profile: match director_profile {
            DirectorProfile::Clean => "clean",
            DirectorProfile::Dynamic => "dynamic",
            DirectorProfile::Soft => "soft",
        }
        .to_string(),
        assets: ProjectAssets {
            source_video: asset_name(&output_path)?,
            mouse_trace: asset_name(&mouse_events_path)?,
            camera_timeline: asset_name(&camera_timeline_path)?,
            edit_timeline: asset_name(&edit_timeline_path)?,
            final_video: asset_name(&final_video_path)?,
        },
    };
    save_edit_timeline(&edit_timeline_path, &EditTimeline::default())?;
    save_project(&project_path, &project)?;
    let recording_origin = Instant::now();
    let log_path = output_path.with_extension("log");
    let log = File::create(&log_path).map_err(|error| format!("无法创建录制日志：{error}"))?;
    let log_copy = log
        .try_clone()
        .map_err(|error| format!("无法初始化录制日志：{error}"))?;

    let offset_x = capture_bounds.x.to_string();
    let offset_y = capture_bounds.y.to_string();
    let video_size = format!("{}x{}", capture_bounds.width, capture_bounds.height);

    let process_result = command(&ffmpeg)
        .args([
            "-hide_banner",
            "-y",
            "-f",
            "gdigrab",
            "-framerate",
            "60",
            "-draw_mouse",
            "1",
            "-rtbufsize",
            "512M",
        ])
        .args(["-offset_x", &offset_x, "-offset_y", &offset_y, "-video_size", &video_size])
        .args(["-i", "desktop", "-c:v", "libx264", "-preset", "veryfast"])
        .args(["-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart"])
        .arg(&output_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::from(log_copy))
        .stderr(Stdio::from(log))
        .spawn();
    let process = process_result.map_err(|error| format!("无法启动屏幕录制：{error}"))?;
    let mouse_tracker = match MouseTrackerHandle::start(
        &mouse_events_path,
        recording_origin,
        capture_bounds,
    ) {
        Ok(tracker) => tracker,
        Err(error) => {
            let mut process = process;
            if let Some(stdin) = process.stdin.as_mut() {
                let _ = stdin.write_all(b"q\n");
            }
            let _ = process.wait();
            return Err(error);
        }
    };

    *active = Some(ActiveRecording {
        process,
        output_path: output_path.clone(),
        mouse_events_path: mouse_events_path.clone(),
        camera_timeline_path: camera_timeline_path.clone(),
        final_video_path: final_video_path.clone(),
        project_path: project_path.clone(),
        project,
        ffmpeg_path: ffmpeg,
        ffprobe_path: ffprobe,
        director_profile,
        mouse_tracker: Some(mouse_tracker),
    });
    *state
        .last_source_video
        .lock()
        .map_err(|_| "最近录制状态不可用".to_string())? = Some(output_path.clone());
    *state
        .last_final_video
        .lock()
        .map_err(|_| "最近视频状态不可用".to_string())? = None;
    Ok(RecorderStatus {
        is_recording: true,
        ffmpeg_available: true,
        output_path: Some(output_path.display().to_string()),
        output_directory: output_directory.display().to_string(),
        mouse_events_path: Some(mouse_events_path.display().to_string()),
        camera_timeline_path: Some(camera_timeline_path.display().to_string()),
        final_video_path: Some(final_video_path.display().to_string()),
        project_path: Some(project_path.display().to_string()),
        director_profile,
    })
}

fn stop_active_recording(
    app: &tauri::AppHandle,
    recording: &mut ActiveRecording,
) -> Result<(), String> {
    emit_processing_progress(app, "saving", 12, "正在保存原始录制…");
    recording.project.state = ProjectState::Processing;
    save_project(&recording.project_path, &recording.project)?;
    let result = stop_and_render(app, recording);
    if let Err(error) = &result {
        recording.project.state = ProjectState::Failed;
        let _ = save_project(&recording.project_path, &recording.project);
        return Err(error.clone());
    }
    recording.project.state = ProjectState::Ready;
    save_project(&recording.project_path, &recording.project)?;
    remove_processing_artifacts(recording)?;
    emit_processing_progress(app, "complete", 100, "自动运镜视频已生成");
    Ok(())
}

fn remove_processing_artifacts(recording: &ActiveRecording) -> Result<(), String> {
    let artifacts = [
        recording.mouse_events_path.clone(),
        recording.camera_timeline_path.clone(),
        recording.output_path.with_extension("edit.json"),
        recording.project_path.clone(),
        recording.output_path.with_extension("log"),
        recording.final_video_path.with_extension("filter.txt"),
        recording.final_video_path.with_extension("render.log"),
    ];

    for path in artifacts {
        if path.exists() {
            fs::remove_file(&path)
                .map_err(|error| format!("无法清理处理文件 {}：{error}", path.display()))?;
        }
    }
    Ok(())
}

fn stop_and_render(
    app: &tauri::AppHandle,
    recording: &mut ActiveRecording,
) -> Result<(), String> {
    let tracker_result = recording
        .mouse_tracker
        .take()
        .map(MouseTrackerHandle::stop)
        .transpose();
    if let Some(stdin) = recording.process.stdin.as_mut() {
        stdin
            .write_all(b"q\n")
            .map_err(|error| format!("无法停止录制：{error}"))?;
        stdin
            .flush()
            .map_err(|error| format!("无法提交停止请求：{error}"))?;
    }
    let status = recording
        .process
        .wait()
        .map_err(|error| format!("等待视频保存失败：{error}"))?;
    if !status.success() {
        return Err(format!(
            "FFmpeg 录制失败，请查看 {}",
            recording.output_path.with_extension("log").display()
        ));
    }
    tracker_result?;
    emit_processing_progress(app, "analyzing", 42, "正在分析鼠标动作…");
    let timeline = generate_camera_timeline(recording)?;
    recording.project.duration_ms = Some(timeline.duration_ms);
    save_project(&recording.project_path, &recording.project)?;
    emit_processing_progress(app, "rendering", 64, "正在生成自动缩放和运镜…");
    let renderer = FfmpegRenderer::new(&recording.ffmpeg_path, &recording.ffprobe_path);
    renderer.render(
        &recording.output_path,
        &timeline,
        &recording.final_video_path,
    )?;
    Ok(())
}

#[tauri::command]
fn open_final_video(state: State<'_, RecorderState>) -> Result<(), String> {
    let path = state
        .last_final_video
        .lock()
        .map_err(|_| "最近视频状态不可用".to_string())?
        .clone()
        .ok_or_else(|| "还没有可以播放的成片".to_string())?;
    if !path.is_file() {
        return Err("成片文件已被移动或删除".to_string());
    }
    command("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(path)
        .spawn()
        .map_err(|error| format!("无法打开视频：{error}"))?;
    Ok(())
}

#[tauri::command]
fn open_source_video(state: State<'_, RecorderState>) -> Result<(), String> {
    let path = state
        .last_source_video
        .lock()
        .map_err(|_| "最近录制状态不可用".to_string())?
        .clone()
        .ok_or_else(|| "还没有可以播放的原始视频".to_string())?;
    if !path.is_file() {
        return Err("原始视频已被移动或删除".to_string());
    }
    command("rundll32.exe")
        .arg("url.dll,FileProtocolHandler")
        .arg(path)
        .spawn()
        .map_err(|error| format!("无法打开原始视频：{error}"))?;
    Ok(())
}

#[tauri::command]
fn reveal_output(state: State<'_, RecorderState>) -> Result<(), String> {
    let final_video = state
        .last_final_video
        .lock()
        .map_err(|_| "最近视频状态不可用".to_string())?
        .clone();
    let output_directory = state
        .output_directory
        .lock()
        .map_err(|_| "保存位置状态不可用".to_string())?
        .clone();
    let source_video = state
        .last_source_video
        .lock()
        .map_err(|_| "最近录制状态不可用".to_string())?
        .clone();

    let mut explorer = command("explorer.exe");
    if let Some(path) = final_video
        .filter(|path| path.is_file())
        .or_else(|| source_video.filter(|path| path.is_file()))
    {
        explorer.arg(format!("/select,{}", path.display()));
    } else {
        explorer.arg(output_directory);
    }
    explorer
        .spawn()
        .map_err(|error| format!("无法打开保存位置：{error}"))?;
    Ok(())
}

fn generate_camera_timeline(recording: &ActiveRecording) -> Result<CameraTimeline, String> {
    let trace = File::open(&recording.mouse_events_path)
        .map_err(|error| format!("无法读取鼠标轨迹：{error}"))?;
    let timeline = SmartFocusEngine::for_profile(recording.director_profile)
        .analyze_trace(BufReader::new(trace))?;
    let output = File::create(&recording.camera_timeline_path)
        .map_err(|error| format!("无法创建镜头时间线：{error}"))?;
    serde_json::to_writer_pretty(BufWriter::new(output), &timeline)
        .map_err(|error| format!("无法保存镜头时间线：{error}"))?;
    Ok(timeline)
}

#[tauri::command]
async fn stop_recording(
    app: tauri::AppHandle,
    state: State<'_, RecorderState>,
) -> Result<RecorderStatus, String> {
    let mut recording = {
        let mut active = state
            .active
            .lock()
            .map_err(|_| "录制状态不可用".to_string())?;
        active
            .take()
            .ok_or_else(|| "当前没有正在进行的录制".to_string())?
    };
    let output_directory = state
        .output_directory
        .lock()
        .map_err(|_| "保存位置状态不可用".to_string())?
        .display()
        .to_string();
    let processing_app = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<RecorderStatus, String> {
        stop_active_recording(&processing_app, &mut recording)?;
        Ok(RecorderStatus {
            is_recording: false,
            ffmpeg_available: true,
            output_path: Some(recording.output_path.display().to_string()),
            output_directory,
            mouse_events_path: Some(recording.mouse_events_path.display().to_string()),
            camera_timeline_path: Some(recording.camera_timeline_path.display().to_string()),
            final_video_path: Some(recording.final_video_path.display().to_string()),
            project_path: Some(recording.project_path.display().to_string()),
            director_profile: recording.director_profile,
        })
    })
    .await
    .map_err(|error| format!("后台视频处理任务失败：{error}"))??;
    if let Some(path) = result.final_video_path.as_ref() {
        *state
            .last_final_video
            .lock()
            .map_err(|_| "最近视频状态不可用".to_string())? = Some(PathBuf::from(path));
    }
    Ok(result)
}

fn main() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(RecorderState::new())
        .invoke_handler(tauri::generate_handler![
            recorder_status,
            set_output_directory,
            set_director_profile,
            open_project,
            load_studio_project,
            update_camera_segment,
            update_video_clips,
            save_studio_edits,
            reset_camera_edits,
            start_recording,
            stop_recording,
            open_final_video,
            open_source_video,
            reveal_output
        ])
        .build(tauri::generate_context!())
        .expect("failed to build FocusFlow");

    app.run(|handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            let state = handle.state::<RecorderState>();
            if let Ok(mut active) = state.active.lock() {
                if let Some(mut recording) = active.take() {
                    let _ = stop_active_recording(handle, &mut recording);
                }
            }
        }
    });
}

