#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_capture;

use focusflow_domain::CameraTimeline;
use focusflow_editor_core::{
    CameraKeyframe, Timeline as EditorTimeline, VideoClip, timeline_from_keyframes,
};
use focusflow_focus_engine::{DirectorProfile, SmartFocusEngine};
use focusflow_platform_windows::{CaptureBounds, MouseTrackerHandle, cursor_render};
use focusflow_project_store::{
    EditOperation, EditTimeline, FocusFlowProject, PROJECT_SCHEMA_VERSION, ProjectAssets,
    ProjectState, VideoClipEdit, load_edit_timeline, load_project, save_edit_timeline,
    save_project,
};
use focusflow_renderer::{FfmpegRenderer, Renderer};
use audio_capture::{AudioCaptureHandle, AudioDeviceInfo};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufRead, BufReader, BufWriter, Write},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::Mutex,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

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
    audio_captures: Vec<AudioCaptureHandle>,
    audio_cache_paths: Vec<PathBuf>,
    audio_lead_seconds: Vec<f64>,
    cursor_options: CursorOptions,
    capture_width: i32,
    capture_height: i32,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CursorPreview {
    path: String,
    hotspot_x: u32,
    hotspot_y: u32,
    width: u32,
    height: u32,
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursorOptions {
    enabled: bool,
    custom_enabled: bool,
    size_percent: u32,
    custom_paths: HashMap<String, String>,
}

impl CursorOptions {
    fn validate(&self) -> Result<(), String> {
        if !(50..=250).contains(&self.size_percent) {
            return Err("鼠标大小必须在 50% 到 250% 之间".to_string());
        }
        for (role, path) in &self.custom_paths {
            if cursor_render::role_id(role).is_none() {
                return Err(format!("未知鼠标指针角色：{role}"));
            }
            let path = std::path::Path::new(path);
            let extension = path.extension().and_then(|value| value.to_str()).unwrap_or_default();
            if !path.is_file()
                || !(extension.eq_ignore_ascii_case("cur") || extension.eq_ignore_ascii_case("ani"))
            {
                return Err(format!("鼠标指针文件无效：{}", path.display()));
            }
        }
        Ok(())
    }
}

#[tauri::command]
fn render_cursor_preview(
    app: tauri::AppHandle,
    role: String,
    custom_path: Option<String>,
    size_percent: u32,
) -> Result<CursorPreview, String> {
    let custom = custom_path.as_deref().map(std::path::Path::new);
    if let Some(path) = custom {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !path.is_file()
            || !(extension.eq_ignore_ascii_case("cur") || extension.eq_ignore_ascii_case("ani"))
        {
            return Err("请选择有效的 .cur 或 .ani 指针文件".to_string());
        }
    }
    let cache = app
        .path()
        .app_cache_dir()
        .map_err(|error| format!("无法读取应用缓存目录：{error}"))?
        .join("cursor-previews");
    fs::create_dir_all(&cache).map_err(|error| format!("无法创建指针预览目录：{error}"))?;
    let safe_role = role.replace(|character: char| !character.is_ascii_alphanumeric(), "_");
    let animated = custom.is_some_and(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ani")));
    let output = cache.join(format!("{safe_role}-{size_percent}.{}", if animated { "gif" } else { "png" }));
    let animation = cursor_render::render_cursor_animation(&role, custom, size_percent)?;
    cursor_render::save_cursor_preview(&animation, &output)?;
    app.asset_protocol_scope()
        .allow_file(&output)
        .map_err(|error| format!("无法授权指针预览：{error}"))?;
    Ok(CursorPreview {
        path: output.display().to_string(),
        hotspot_x: animation.hotspot_x,
        hotspot_y: animation.hotspot_y,
        width: animation.width,
        height: animation.height,
    })
}

#[tauri::command]
fn choose_cursor_file(app: tauri::AppHandle, initial_directory: Option<String>) -> Option<String> {
    let mut picker = app.dialog().file().add_filter("Windows Cursor", &["cur", "ani"]);
    if let Some(directory) = initial_directory.filter(|value| std::path::Path::new(value).is_dir()) {
        picker = picker.set_directory(directory);
    }
    picker.blocking_pick_file()?.into_path().ok().map(|path| path.display().to_string())
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

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AudioOptions {
    system_audio: bool,
    microphone: bool,
    microphone_device_id: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioCapabilities {
    system_audio_available: bool,
    microphones: Vec<AudioDeviceInfo>,
}

#[tauri::command]
async fn list_audio_devices() -> Result<AudioCapabilities, String> {
    tauri::async_runtime::spawn_blocking(|| {
        Ok(AudioCapabilities {
            system_audio_available: audio_capture::system_audio_available()?,
            microphones: audio_capture::list_microphones()?,
        })
    })
    .await
    .map_err(|error| format!("检测音频设备失败：{error}"))?
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
    audio_options: AudioOptions,
    cursor_options: CursorOptions,
    app: tauri::AppHandle,
    state: State<'_, RecorderState>,
) -> Result<RecorderStatus, String> {
    cursor_options.validate()?;
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
    let system_audio_path = output_path.with_extension("system.f32");
    let microphone_path = output_path.with_extension("microphone.f32");
    let mut audio_captures = Vec::new();
    if audio_options.system_audio {
        audio_captures.push(audio_capture::start_system_audio(system_audio_path.clone())?);
    }
    if audio_options.microphone {
        match audio_capture::start_microphone(
            microphone_path.clone(),
            audio_options.microphone_device_id.clone(),
        ) {
            Ok(capture) => audio_captures.push(capture),
            Err(error) => {
                for capture in audio_captures {
                    let _ = capture.stop();
                }
                let _ = fs::remove_file(&system_audio_path);
                return Err(error);
            }
        }
    }
    let recording_origin = Instant::now();
    let audio_lead_seconds = audio_captures
        .iter()
        .map(|capture| {
            recording_origin
                .checked_duration_since(capture.started_at)
                .unwrap_or_default()
                .as_secs_f64()
        })
        .collect::<Vec<_>>();
    let audio_cache_paths = audio_captures
        .iter()
        .map(|capture| capture.path.clone())
        .collect::<Vec<_>>();
    let log_path = output_path.with_extension("log");
    let log = File::create(&log_path).map_err(|error| format!("无法创建录制日志：{error}"))?;
    let log_copy = log
        .try_clone()
        .map_err(|error| format!("无法初始化录制日志：{error}"))?;

    let offset_x = capture_bounds.x.to_string();
    let offset_y = capture_bounds.y.to_string();
    let video_size = format!("{}x{}", capture_bounds.width, capture_bounds.height);

    let draw_mouse = if cursor_options.enabled { "0" } else { "1" };
    let process_result = command(&ffmpeg)
        .args([
            "-hide_banner",
            "-y",
            "-f",
            "gdigrab",
            "-framerate",
            "60",
            "-draw_mouse",
            draw_mouse,
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
    let process = match process_result {
        Ok(process) => process,
        Err(error) => {
            for capture in audio_captures {
                let _ = capture.stop();
            }
            return Err(format!("无法启动屏幕录制：{error}"));
        }
    };
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
            for capture in audio_captures {
                let _ = capture.stop();
            }
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
        audio_captures,
        audio_cache_paths,
        audio_lead_seconds,
        cursor_options,
        capture_width,
        capture_height,
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
    let mut artifacts = vec![
        recording.mouse_events_path.clone(),
        recording.camera_timeline_path.clone(),
        recording.output_path.with_extension("edit.json"),
        recording.project_path.clone(),
        recording.output_path.with_extension("log"),
        recording.final_video_path.with_extension("filter.txt"),
        recording.final_video_path.with_extension("render.log"),
        recording.output_path.with_extension("audio.mp4"),
    ];
    artifacts.extend(recording.audio_cache_paths.iter().cloned());

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
    for capture in recording.audio_captures.drain(..) {
        capture.stop()?;
    }
    mux_recorded_audio(recording)?;
    if recording.cursor_options.enabled {
        emit_processing_progress(app, "cursor", 32, "正在应用鼠标指针样式…");
        apply_cursor_overlay(recording)?;
    }
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

#[derive(Deserialize)]
struct CursorTracePoint {
    #[serde(rename = "type")]
    line_type: String,
    time_ms: Option<u64>,
    normalized_x: Option<f64>,
    normalized_y: Option<f64>,
    cursor_role: Option<String>,
}

fn apply_cursor_overlay(recording: &ActiveRecording) -> Result<(), String> {
    let trace = File::open(&recording.mouse_events_path)
        .map_err(|error| format!("无法读取鼠标轨迹：{error}"))?;
    let mut points = Vec::new();
    for line in BufReader::new(trace).lines() {
        let line = line.map_err(|error| format!("无法读取鼠标轨迹：{error}"))?;
        let Ok(point) = serde_json::from_str::<CursorTracePoint>(&line) else {
            continue;
        };
        if point.line_type != "mouse" {
            continue;
        }
        if let (Some(time_ms), Some(x), Some(y)) =
            (point.time_ms, point.normalized_x, point.normalized_y)
        {
            points.push((
                time_ms,
                x.clamp(0.0, 1.0),
                y.clamp(0.0, 1.0),
                point.cursor_role.unwrap_or_else(|| "normal".to_string()),
            ));
        }
    }
    if points.is_empty() {
        return Ok(());
    }
    // The tracker can report far more often than the 60 fps video can display.
    // Keep only the latest position for each frame to avoid a huge sendcmd script.
    let mut sampled = Vec::with_capacity(points.len().min(36_000));
    for point in points {
        let bucket = point.0 / 16;
        if sampled.last().is_some_and(|previous: &(u64, f64, f64, String)| previous.0 / 16 == bucket) {
            *sampled.last_mut().expect("sample exists") = point;
        } else {
            sampled.push(point);
        }
    }
    let points = sampled;

    let work_dir = recording
        .output_path
        .parent()
        .ok_or_else(|| "无法定位录制目录".to_string())?;
    let stem = recording
        .output_path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法生成鼠标合成文件名".to_string())?;
    let cursor_dir = work_dir.join(format!(".{stem}-cursors"));
    fs::create_dir_all(&cursor_dir).map_err(|error| format!("无法创建鼠标合成目录：{error}"))?;

    let mut roles = points
        .iter()
        .map(|(_, _, _, role)| role.clone())
        .collect::<Vec<_>>();
    roles.sort();
    roles.dedup();
    roles.retain(|role| cursor_render::role_id(role).is_some());
    if roles.is_empty() {
        roles.push("normal".to_string());
    }

    struct OverlayAsset {
        input: PathBuf,
        animated: bool,
        hotspot_x: u32,
        hotspot_y: u32,
    }
    let mut rendered: HashMap<String, OverlayAsset> = HashMap::new();
    for role in &roles {
        let custom = recording
            .cursor_options
            .custom_enabled
            .then(|| recording.cursor_options.custom_paths.get(role))
            .flatten()
            .map(std::path::Path::new);
        let animation = cursor_render::render_cursor_animation(role, custom, recording.cursor_options.size_percent)?;
        if animation.frames.len() == 1 {
            let png = cursor_dir.join(format!("{role}.png"));
            animation.frames[0].image.save(&png).map_err(|error| format!("无法保存鼠标指针帧：{error}"))?;
            rendered.insert(role.clone(), OverlayAsset { input: png, animated: false, hotspot_x: animation.hotspot_x, hotspot_y: animation.hotspot_y });
        } else {
            let role_dir = cursor_dir.join(role);
            fs::create_dir_all(&role_dir).map_err(|error| format!("无法创建 ANI 帧目录：{error}"))?;
            let concat = role_dir.join("animation.ffconcat");
            let mut list = BufWriter::new(File::create(&concat).map_err(|error| format!("无法创建 ANI 时间线：{error}"))?);
            writeln!(list, "ffconcat version 1.0").map_err(|error| format!("无法写入 ANI 时间线：{error}"))?;
            let mut last = None;
            for (index, frame) in animation.frames.iter().enumerate() {
                let frame_path = role_dir.join(format!("frame-{index:04}.png"));
                frame.image.save(&frame_path).map_err(|error| format!("无法保存 ANI 帧：{error}"))?;
                let escaped = frame_path.display().to_string().replace('\\', "/").replace('\'', "'\\''");
                writeln!(list, "file '{escaped}'\nduration {:.6}", f64::from(frame.duration_ms) / 1000.0)
                    .map_err(|error| format!("无法写入 ANI 时间线：{error}"))?;
                last = Some(escaped);
            }
            if let Some(last) = last { writeln!(list, "file '{last}'").map_err(|error| format!("无法写入 ANI 时间线：{error}"))?; }
            list.flush().map_err(|error| format!("无法保存 ANI 时间线：{error}"))?;
            rendered.insert(role.clone(), OverlayAsset { input: concat, animated: true, hotspot_x: animation.hotspot_x, hotspot_y: animation.hotspot_y });
        }
    }

    let commands_path = cursor_dir.join("commands.txt");
    let mut commands = BufWriter::new(
        File::create(&commands_path).map_err(|error| format!("无法创建鼠标合成指令：{error}"))?,
    );
    let mut previous_role: Option<String> = None;
    for (time_ms, normalized_x, normalized_y, recorded_role) in points {
        let role = if rendered.contains_key(&recorded_role) {
            recorded_role
        } else {
            "normal".to_string()
        };
        let info = rendered
            .get(&role)
            .ok_or_else(|| "无法读取鼠标图像信息".to_string())?;
        let x = (normalized_x * f64::from(recording.capture_width) - f64::from(info.hotspot_x))
            .round() as i32;
        let y = (normalized_y * f64::from(recording.capture_height) - f64::from(info.hotspot_y))
            .round() as i32;
        let timestamp = time_ms as f64 / 1000.0;
        if previous_role.as_deref() != Some(&role) {
            if let Some(previous) = previous_role.as_deref() {
                writeln!(commands, "{timestamp:.6} overlay@{previous} x -10000;")
                    .map_err(|error| format!("无法写入鼠标合成指令：{error}"))?;
            }
            previous_role = Some(role.clone());
        }
        writeln!(commands, "{timestamp:.6} overlay@{role} x {x};")
            .and_then(|_| writeln!(commands, "{timestamp:.6} overlay@{role} y {y};"))
            .map_err(|error| format!("无法写入鼠标合成指令：{error}"))?;
    }
    commands.flush().map_err(|error| format!("无法保存鼠标合成指令：{error}"))?;

    let command_name = commands_path
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| "无法读取鼠标合成指令文件名".to_string())?;
    let mut filters = vec![format!("[0:v]sendcmd=f='{command_name}'[cursorbase]")];
    let mut current = "cursorbase".to_string();
    for (index, role) in roles.iter().enumerate() {
        let next = format!("cursor{index}");
        filters.push(format!(
            "[{current}][{}:v]overlay@{role}=x=-10000:y=-10000:shortest=1:eof_action=repeat[{next}]",
            index + 1
        ));
        current = next;
    }
    let filter = filters.join(";");
    let composited_path = cursor_dir.join("source-with-cursor.mp4");
    let mut process = command(&recording.ffmpeg_path);
    process.current_dir(&cursor_dir).args(["-hide_banner", "-y", "-i"])
        .arg(&recording.output_path);
    for role in &roles {
        let asset = rendered.get(role).ok_or_else(|| "无法读取鼠标图像".to_string())?;
        if asset.animated {
            process.args(["-stream_loop", "-1", "-f", "concat", "-safe", "0", "-i"]).arg(&asset.input);
        } else {
            process.args(["-loop", "1", "-i"]).arg(&asset.input);
        }
    }
    process.args(["-filter_complex", &filter, "-map", &format!("[{current}]"), "-map", "0:a?"]);
    if h264_nvenc_available(&recording.ffmpeg_path) {
        process.args(["-c:v", "h264_nvenc", "-preset", "p4", "-tune", "hq", "-rc", "vbr", "-cq", "19", "-b:v", "0"]);
    } else {
        process.args(["-c:v", "libx264", "-preset", "veryfast", "-crf", "18"]);
    }
    let status = process
        .args(["-pix_fmt", "yuv420p", "-c:a", "copy", "-shortest", "-movflags", "+faststart"])
        .arg(&composited_path)
        .stdin(Stdio::null())
        .status()
        .map_err(|error| format!("无法启动鼠标指针合成：{error}"))?;
    if !status.success() {
        return Err("鼠标指针合成失败".to_string());
    }
    fs::remove_file(&recording.output_path)
        .map_err(|error| format!("无法替换原始视频：{error}"))?;
    fs::rename(&composited_path, &recording.output_path)
        .map_err(|error| format!("无法保存带鼠标指针的原始视频：{error}"))?;
    let _ = fs::remove_dir_all(&cursor_dir);
    Ok(())
}

fn h264_nvenc_available(ffmpeg: &PathBuf) -> bool {
    command(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=size=64x64:rate=1", "-frames:v", "1", "-c:v", "h264_nvenc", "-f", "null", "NUL"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn mux_recorded_audio(recording: &ActiveRecording) -> Result<(), String> {
    if recording.audio_cache_paths.is_empty() {
        return Ok(());
    }
    let muxed_path = recording.output_path.with_extension("audio.mp4");
    let mut process = command(&recording.ffmpeg_path);
    process.args(["-hide_banner", "-y", "-i"]).arg(&recording.output_path);
    for path in &recording.audio_cache_paths {
        process
            .args(["-f", "f32le", "-ar", "44100", "-ac", "2", "-i"])
            .arg(path);
    }

    let mut prepared = Vec::new();
    let mut filters = Vec::new();
    for (index, lead) in recording.audio_lead_seconds.iter().enumerate() {
        let label = format!("a{}", index + 1);
        filters.push(format!(
            "[{}:a]atrim=start={lead:.6},asetpts=PTS-STARTPTS[{label}]",
            index + 1
        ));
        prepared.push(format!("[{label}]"));
    }
    if prepared.len() == 1 {
        filters.push(format!("{}aresample=async=1:first_pts=0,apad[aout]", prepared[0]));
    } else {
        filters.push(format!(
            "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0,alimiter=limit=0.95,aresample=async=1:first_pts=0,apad[aout]",
            prepared.join(""),
            prepared.len()
        ));
    }
    let filter = filters.join(";");
    let status = process
        .args(["-filter_complex", &filter, "-map", "0:v:0", "-map", "[aout]"])
        .args(["-c:v", "copy", "-c:a", "aac", "-b:a", "192k", "-shortest", "-movflags", "+faststart"])
        .arg(&muxed_path)
        .stdin(Stdio::null())
        .status()
        .map_err(|error| format!("无法启动音视频合并：{error}"))?;
    if !status.success() {
        return Err("系统声音或麦克风与视频合并失败".to_string());
    }
    fs::remove_file(&recording.output_path)
        .map_err(|error| format!("无法替换原始视频：{error}"))?;
    fs::rename(&muxed_path, &recording.output_path)
        .map_err(|error| format!("无法保存带声音的原始视频：{error}"))?;
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
            list_audio_devices,
            render_cursor_preview,
            choose_cursor_file,
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

