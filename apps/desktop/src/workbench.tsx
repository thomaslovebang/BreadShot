import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  MAX_ZOOM,
  MIN_ZOOM,
  cloneKeyframes,
  insertKeyframe,
  keyframesFromTimeline,
  moveKeyframe,
  removeKeyframe,
  resetKeyframePose,
  sameKeyframes,
  samplePose,
  setKeyframePose,
  zoomToTopPercent,
  type CameraKeyframe,
  type CameraTimeline,
  type FocusReason,
} from "./camera-edit";

export type VideoClip = { id: string; source_start_ms: number; source_end_ms: number };

export type StudioProject = {
  projectPath: string;
  title: string;
  finalVideoPath: string;
  sourceVideoPath: string;
  durationMs: number;
  cameraTimeline: CameraTimeline;
  baselineCameraTimeline: CameraTimeline;
  videoClips: VideoClip[];
  cameraSegmentSourceIndices: number[];
  previewRevision: number;
};

type WorkbenchProps = {
  studio: StudioProject;
  message: string;
  autoPlay?: boolean;
  onBack: () => void;
  onStudioChange: (studio: StudioProject) => void;
  onMessage: (message: string) => void;
};

function mediaUrl(path: string) {
  try {
    return convertFileSrc(path);
  } catch {
    return path;
  }
}

const REASON_LABEL: Record<FocusReason, string> = {
  click: "点击聚焦",
  dwell: "停留聚焦",
  transition: "镜头移动",
  full_frame: "全屏",
};

function formatMilliseconds(milliseconds: number) {
  const totalSeconds = Math.max(0, milliseconds) / 1000;
  const minutes = Math.floor(totalSeconds / 60);
  const seconds = Math.floor(totalSeconds % 60);
  const tenths = Math.floor((totalSeconds % 1) * 10);
  return `${minutes}:${seconds.toString().padStart(2, "0")}.${tenths}`;
}

function nearestKeyframe(frames: CameraKeyframe[], timeMs: number) {
  let best = 0;
  let bestDistance = Number.POSITIVE_INFINITY;
  frames.forEach((frame, index) => {
    const distance = Math.abs(frame.time_ms - timeMs);
    if (distance < bestDistance) {
      best = index;
      bestDistance = distance;
    }
  });
  return best;
}

export function Workbench({ studio, message, autoPlay = false, onBack, onStudioChange, onMessage }: WorkbenchProps) {
  const initialFrames = keyframesFromTimeline(studio.cameraTimeline);
  const baseline = useMemo(
    () => keyframesFromTimeline(studio.baselineCameraTimeline),
    [studio.baselineCameraTimeline],
  );
  const [keyframes, setKeyframes] = useState(initialFrames);
  const [selected, setSelected] = useState<number | null>(null);
  const [playheadMs, setPlayheadMs] = useState(0);
  const [timelineZoom, setTimelineZoom] = useState(1);
  const [playing, setPlaying] = useState(false);
  const [previewMode, setPreviewMode] = useState<"final" | "source">("final");
  const [aspect, setAspect] = useState(16 / 9);
  const [rendering, setRendering] = useState(false);
  const [reload, setReload] = useState(0);
  const [cameraUndo, setCameraUndo] = useState<CameraKeyframe[][]>([]);
  const [cameraDirty, setCameraDirty] = useState(false);
  const [videoClips, setVideoClips] = useState(studio.videoClips);
  const [selectedClip, setSelectedClip] = useState<string | null>(studio.videoClips[0]?.id ?? null);
  const [clipUndo, setClipUndo] = useState<VideoClip[][]>([]);
  const [clipRedo, setClipRedo] = useState<VideoClip[][]>([]);
  const keyframesRef = useRef(initialFrames);
  const durationRef = useRef(1);
  const playheadRef = useRef(0);
  const sliderOrigin = useRef<CameraKeyframe[] | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const previewFrameRef = useRef<HTMLDivElement>(null);
  const autoPlayRef = useRef(autoPlay);
  autoPlayRef.current = autoPlay;

  const duration = Math.max(studio.durationMs, keyframes.at(-1)?.time_ms ?? 0, 1);
  durationRef.current = duration;
  playheadRef.current = playheadMs;
  const selectedFrame = selected == null ? null : keyframes[selected] ?? null;
  const reticlePose = selectedFrame?.pose ?? samplePose(keyframes, playheadMs);
  const showReticle = previewMode === "source" && reticlePose.zoom > 1.01;
  const crop = 1 / Math.max(reticlePose.zoom, MIN_ZOOM);
  const sourceReady = studio.sourceVideoPath.length > 0;
  const previewPath = previewMode === "source" && sourceReady ? studio.sourceVideoPath : studio.finalVideoPath;
  const curve = keyframes
    .map((frame) => {
      const x = (frame.time_ms / duration) * 1000;
      return `${x.toFixed(2)},${zoomToTopPercent(frame.pose.zoom).toFixed(2)}`;
    })
    .join(" ");
  const clipDuration = videoClips.reduce((sum, clip) => sum + clip.source_end_ms - clip.source_start_ms, 0);

  function rememberKeyframes(next: CameraKeyframe[]) {
    keyframesRef.current = next;
    setKeyframes(next);
  }

  function commitCamera(origin: CameraKeyframe[]) {
    if (sameKeyframes(origin, keyframesRef.current)) return;
    setCameraUndo((stack) => [...stack, origin].slice(-50));
    setCameraDirty(true);
  }

  function seekTo(milliseconds: number) {
    const clamped = Math.max(0, Math.min(milliseconds, duration));
    if (videoRef.current) videoRef.current.currentTime = clamped / 1000;
    setPlayheadMs(clamped);
  }

  function togglePlay() {
    const video = videoRef.current;
    if (!video || rendering) return;
    if (video.paused) {
      void video.play().catch(() => onMessage("无法播放预览"));
    } else {
      video.pause();
    }
  }

  function scrubFromTrack(event: React.PointerEvent<HTMLDivElement>) {
    if ((event.target as HTMLElement).closest("button")) return;
    const track = event.currentTarget;
    const update = (clientX: number) => {
      const rect = track.getBoundingClientRect();
      seekTo(((clientX - rect.left) / rect.width) * durationRef.current);
    };
    update(event.clientX);
    const move = (pointer: PointerEvent) => update(pointer.clientX);
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function onKeyframePointerDown(event: React.PointerEvent<HTMLButtonElement>, index: number) {
    event.stopPropagation();
    event.preventDefault();
    const handle = event.currentTarget;
    const lane = handle.closest(".camera-lane");
    if (!(lane instanceof HTMLElement)) return;
    const rect = lane.getBoundingClientRect();
    const originFrames = cloneKeyframes(keyframesRef.current);
    const origin = originFrames[index];
    if (!origin) return;
    setSelected(index);
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startY = event.clientY;
    const move = (pointer: PointerEvent) => {
      const dt = ((pointer.clientX - startX) / rect.width) * durationRef.current;
      const dz = -((pointer.clientY - startY) / rect.height) * (MAX_ZOOM - MIN_ZOOM);
      const next = moveKeyframe(originFrames, index, origin.time_ms + dt, origin.pose.zoom + dz);
      if (next) rememberKeyframes(next);
    };
    const up = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      commitCamera(originFrames);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
  }

  function onFocusPointerDown(event: React.PointerEvent<HTMLButtonElement>) {
    event.stopPropagation();
    event.preventDefault();
    const index = selected ?? nearestKeyframe(keyframesRef.current, playheadRef.current);
    const originFrames = cloneKeyframes(keyframesRef.current);
    const origin = originFrames[index];
    const frame = previewFrameRef.current;
    if (!origin || !frame) return;
    setSelected(index);
    const rect = frame.getBoundingClientRect();
    const handle = event.currentTarget;
    handle.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startY = event.clientY;
    const move = (pointer: PointerEvent) => {
      const next = setKeyframePose(originFrames, index, {
        zoom: origin.pose.zoom,
        center: {
          x: origin.pose.center.x + (pointer.clientX - startX) / rect.width,
          y: origin.pose.center.y + (pointer.clientY - startY) / rect.height,
        },
      });
      if (next) rememberKeyframes(next);
    };
    const up = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", up);
      commitCamera(originFrames);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", up);
  }

  function addKeyframeAt(timeMs: number) {
    const origin = cloneKeyframes(keyframesRef.current);
    const inserted = insertKeyframe(origin, timeMs);
    if (!inserted) {
      onMessage("请把播放头放在两个关键帧之间再添加");
      return;
    }
    rememberKeyframes(inserted.frames);
    setSelected(inserted.index);
    setCameraUndo((stack) => [...stack, origin].slice(-50));
    setCameraDirty(true);
    onMessage("已添加镜头关键帧。导出后才会写入成片");
  }

  function deleteSelectedKeyframe() {
    if (selected == null) return;
    const origin = cloneKeyframes(keyframesRef.current);
    const next = removeKeyframe(origin, selected);
    if (!next) {
      onMessage("首尾关键帧需要保留，才能覆盖整段画面");
      return;
    }
    rememberKeyframes(next);
    setSelected(null);
    setCameraUndo((stack) => [...stack, origin].slice(-50));
    setCameraDirty(true);
    onMessage("已删除镜头关键帧。导出后才会写入成片");
  }

  function resetSelectedKeyframe() {
    if (selected == null) return;
    const origin = cloneKeyframes(keyframesRef.current);
    const next = resetKeyframePose(origin, selected, baseline);
    if (!next || sameKeyframes(origin, next)) {
      onMessage("这个关键帧已经与自动镜头一致");
      return;
    }
    rememberKeyframes(next);
    setCameraUndo((stack) => [...stack, origin].slice(-50));
    setCameraDirty(true);
    onMessage("已把关键帧恢复到自动镜头在该时间的缩放和焦点");
  }

  function undoCamera() {
    const previous = cameraUndo.at(-1);
    if (!previous) return;
    rememberKeyframes(previous);
    setCameraDirty(cameraUndo.length > 1);
    setSelected(null);
    setCameraUndo((stack) => stack.slice(0, -1));
  }

  function updateSelectedPose(partial: Partial<CameraKeyframe["pose"]> & { centerX?: number; centerY?: number }) {
    if (selected == null) return;
    const current = keyframesRef.current[selected];
    if (!current) return;
    const next = setKeyframePose(keyframesRef.current, selected, {
      zoom: partial.zoom ?? current.pose.zoom,
      center: {
        x: partial.centerX ?? current.pose.center.x,
        y: partial.centerY ?? current.pose.center.y,
      },
    });
    if (next) {
      rememberKeyframes(next);
      setCameraDirty(true);
    }
  }

  function snapshotSliders() {
    sliderOrigin.current = cloneKeyframes(keyframesRef.current);
  }

  function commitSliders() {
    const origin = sliderOrigin.current;
    sliderOrigin.current = null;
    if (!origin) return;
    commitCamera(origin);
  }

  function releasePreview() {
    const video = videoRef.current;
    video?.pause();
    video?.removeAttribute("src");
    video?.load();
  }

  async function exportCamera() {
    if (rendering || keyframes.length < 2) return;
    setRendering(true);
    onMessage("正在按手动镜头重新导出成片…");
    releasePreview();
    try {
      const updated = await invoke<StudioProject>("save_studio_edits", {
        projectPath: studio.projectPath,
        keyframes,
      });
      rememberKeyframes(keyframesFromTimeline(updated.cameraTimeline));
      setCameraUndo([]);
      setCameraDirty(false);
      setRendering(false);
      onMessage("镜头修改已写入项目，成片已重新生成");
      onStudioChange(updated);
    } catch (error) {
      setReload((value) => value + 1);
      onMessage(`镜头导出失败：${String(error)}`);
      setRendering(false);
    }
  }

  async function restoreAutomaticCamera() {
    if (rendering) return;
    setRendering(true);
    onMessage("正在恢复自动镜头并重新导出…");
    releasePreview();
    try {
      const updated = await invoke<StudioProject>("reset_camera_edits", {
        projectPath: studio.projectPath,
      });
      rememberKeyframes(keyframesFromTimeline(updated.cameraTimeline));
      setCameraUndo([]);
      setCameraDirty(false);
      setSelected(null);
      setRendering(false);
      onMessage("已恢复自动镜头，成片已重新生成");
      onStudioChange(updated);
    } catch (error) {
      setReload((value) => value + 1);
      onMessage(`恢复自动镜头失败：${String(error)}`);
      setRendering(false);
    }
  }

  function commitClips(next: VideoClip[]) {
    setClipUndo((history) => [...history, videoClips]);
    setClipRedo([]);
    setVideoClips(next);
  }

  function splitClipAtPlayhead() {
    let cursor = 0;
    const index = videoClips.findIndex((clip) => {
      const end = cursor + clip.source_end_ms - clip.source_start_ms;
      const contains = playheadMs > cursor && playheadMs < end;
      cursor = end;
      return contains;
    });
    if (index < 0) {
      onMessage("请把播放头移动到片段内部再分割");
      return;
    }
    const before = videoClips.slice(0, index).reduce((sum, clip) => sum + clip.source_end_ms - clip.source_start_ms, 0);
    const clip = videoClips[index];
    const sourceSplit = clip.source_start_ms + Math.round(playheadMs - before);
    const right: VideoClip = { id: `clip-${Date.now()}`, source_start_ms: sourceSplit, source_end_ms: clip.source_end_ms };
    const next = [...videoClips];
    next.splice(index, 1, { ...clip, source_end_ms: sourceSplit }, right);
    commitClips(next);
    setSelectedClip(right.id);
    onMessage("已分割片段。生成片段后会按自动镜头重新对齐");
  }

  function deleteSelectedClip() {
    if (!selectedClip || videoClips.length <= 1) {
      onMessage("时间线必须至少保留一个片段");
      return;
    }
    const next = videoClips.filter((clip) => clip.id !== selectedClip);
    commitClips(next);
    setSelectedClip(next[0]?.id ?? null);
    onMessage("已执行 Ripple 删除。生成片段后才会写入成片");
  }

  function moveSelectedClip(direction: -1 | 1) {
    const index = videoClips.findIndex((clip) => clip.id === selectedClip);
    const target = index + direction;
    if (index < 0 || target < 0 || target >= videoClips.length) return;
    const next = [...videoClips];
    const [clip] = next.splice(index, 1);
    next.splice(target, 0, clip);
    commitClips(next);
    onMessage("片段已移动并自动吸附到相邻片段");
  }

  function undoClips() {
    const previous = clipUndo.at(-1);
    if (!previous) return;
    setClipRedo((history) => [...history, videoClips]);
    setVideoClips(previous);
    setClipUndo((history) => history.slice(0, -1));
  }

  function redoClips() {
    const next = clipRedo.at(-1);
    if (!next) return;
    setClipUndo((history) => [...history, videoClips]);
    setVideoClips(next);
    setClipRedo((history) => history.slice(0, -1));
  }

  async function renderVideoTimeline() {
    if (rendering || cameraDirty) return;
    setRendering(true);
    onMessage("正在应用片段并按自动镜头重新生成成片…");
    releasePreview();
    try {
      const updated = await invoke<StudioProject>("update_video_clips", {
        projectPath: studio.projectPath,
        clips: videoClips,
      });
      rememberKeyframes(keyframesFromTimeline(updated.cameraTimeline));
      setVideoClips(updated.videoClips);
      setClipUndo([]);
      setClipRedo([]);
      setCameraUndo([]);
      setCameraDirty(false);
      setPlayheadMs(0);
      setRendering(false);
      onMessage("片段已保存。镜头已按自动路径重新对齐");
      onStudioChange(updated);
    } catch (error) {
      setReload((value) => value + 1);
      onMessage(`片段生成失败：${String(error)}`);
      setRendering(false);
    }
  }

  const actions = useRef({ togglePlay, undoCamera });
  actions.current = { togglePlay, undoCamera };
  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      const target = event.target;
      if (target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return;
      if (event.code === "Space") {
        event.preventDefault();
        actions.current.togglePlay();
      }
      if (event.key === "z" && (event.metaKey || event.ctrlKey) && !event.shiftKey) {
        event.preventDefault();
        actions.current.undoCamera();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const rulerMarks = [0, 0.25, 0.5, 0.75, 1];
  const safeClipDuration = Math.max(clipDuration, 1);

  return (
    <section className="studio-workspace">
      <div className="studio-heading">
        <div>
          <span className="eyebrow">WORKBENCH · PHASE 9</span>
          <h1>{studio.title}</h1>
          <p className="workbench-message">{message}</p>
        </div>
        <div className="studio-heading-actions">
          <div className="preview-toggle" role="group" aria-label="预览来源">
            <button type="button" className={previewMode === "final" ? "active" : ""} onClick={() => setPreviewMode("final")}>成片</button>
            <button type="button" className={previewMode === "source" ? "active" : ""} disabled={!sourceReady || rendering} onClick={() => setPreviewMode("source")}>原片</button>
          </div>
          <div className="studio-time">
            <strong>{formatMilliseconds(playheadMs)}</strong>
            <span>/ {formatMilliseconds(duration)}</span>
          </div>
          <button className="secondary-button" type="button" onClick={onBack}>返回录制</button>
        </div>
      </div>

      <div className="preview-stage">
        <div className="preview-frame" ref={previewFrameRef} style={{ aspectRatio: String(aspect) }}>
          <video
            key={`${previewPath}-${studio.previewRevision}-${reload}`}
            ref={videoRef}
            src={mediaUrl(previewPath)}
            preload="metadata"
            onPlay={() => setPlaying(true)}
            onPause={() => setPlaying(false)}
            onClick={togglePlay}
            onTimeUpdate={(event) => setPlayheadMs(event.currentTarget.currentTime * 1000)}
            onLoadedMetadata={(event) => {
              const video = event.currentTarget;
              video.volume = 0.8;
              if (video.videoWidth > 0 && video.videoHeight > 0) setAspect(video.videoWidth / video.videoHeight);
              if (playheadRef.current > 0) video.currentTime = playheadRef.current / 1000;
              if (autoPlayRef.current) void video.play().catch(() => onMessage("预览已打开，点击画面即可播放"));
            }}
          />
          {showReticle && (
            <div
              className="focus-reticle"
              style={{ left: `${(reticlePose.center.x - crop / 2) * 100}%`, top: `${(reticlePose.center.y - crop / 2) * 100}%`, width: `${crop * 100}%`, height: `${crop * 100}%` }}
            >
              <span>对焦</span>
              <button className="focus-handle" type="button" aria-label="拖动对焦中心" disabled={rendering} onPointerDown={onFocusPointerDown} />
            </div>
          )}
        </div>
      </div>

      <section className="timeline-panel">
        <div className="timeline-toolbar">
          <div className="transport">
            <button type="button" onClick={togglePlay} disabled={rendering}>{playing ? "暂停" : "播放"}</button>
            <button type="button" onClick={() => seekTo(0)} disabled={rendering}>回到开头</button>
          </div>
          <div className="edit-tools">
            <button type="button" onClick={undoCamera} disabled={!cameraUndo.length || rendering}>撤销镜头</button>
            <button type="button" onClick={() => addKeyframeAt(playheadMs)} disabled={rendering}>添加关键帧</button>
            <button type="button" onClick={deleteSelectedKeyframe} disabled={selected == null || rendering}>删除关键帧</button>
            <button type="button" onClick={resetSelectedKeyframe} disabled={selected == null || rendering}>重置关键帧</button>
            <button type="button" onClick={() => void restoreAutomaticCamera()} disabled={rendering}>恢复自动镜头</button>
            <button className="generate-edit" type="button" onClick={() => void exportCamera()} disabled={!cameraDirty || rendering || keyframes.length < 2}>
              {rendering ? "导出中…" : "导出镜头"}
            </button>
          </div>
          <div className="edit-tools clip-tools">
            <button type="button" onClick={undoClips} disabled={!clipUndo.length || rendering}>撤销片段</button>
            <button type="button" onClick={redoClips} disabled={!clipRedo.length || rendering}>重做</button>
            <button type="button" onClick={splitClipAtPlayhead} disabled={rendering || cameraDirty}>分割</button>
            <button type="button" onClick={() => moveSelectedClip(-1)} disabled={!selectedClip || rendering || cameraDirty}>左移</button>
            <button type="button" onClick={() => moveSelectedClip(1)} disabled={!selectedClip || rendering || cameraDirty}>右移</button>
            <button type="button" onClick={deleteSelectedClip} disabled={!selectedClip || videoClips.length <= 1 || rendering || cameraDirty}>删除</button>
            <button className="generate-edit" type="button" onClick={() => void renderVideoTimeline()} disabled={!clipUndo.length || rendering || cameraDirty} title={cameraDirty ? "请先导出或撤销镜头修改" : "生成片段会按自动镜头重新对齐"}>
              生成片段
            </button>
          </div>
          <div className="zoom-controls">
            <button type="button" onClick={() => setTimelineZoom((value) => Math.max(1, value - 0.5))}>−</button>
            <span>{Math.round(timelineZoom * 100)}%</span>
            <button type="button" onClick={() => setTimelineZoom((value) => Math.min(8, value + 0.5))}>＋</button>
          </div>
        </div>

        <div className="timeline-body">
          <div className="track-labels" aria-hidden="true">
            <span />
            <span className="track-label video">视频</span>
            <span className="track-label camera">镜头</span>
            <span className="track-label audio">音频</span>
          </div>
          <div className="timeline-scroll">
            <div className="timeline-track" style={{ width: `${timelineZoom * 100}%` }} onPointerDown={scrubFromTrack}>
              <div className="time-ruler">
                {rulerMarks.map((ratio) => (
                  <span key={ratio} style={{ left: `${ratio * 100}%` }}>{formatMilliseconds(duration * ratio)}</span>
                ))}
              </div>
              <div className="video-lane">
                {videoClips.map((clip, index) => {
                  const start = videoClips.slice(0, index).reduce((sum, item) => sum + item.source_end_ms - item.source_start_ms, 0);
                  const width = clip.source_end_ms - clip.source_start_ms;
                  return (
                    <button
                      type="button"
                      key={clip.id}
                      className={`video-clip ${selectedClip === clip.id ? "selected" : ""}`}
                      style={{ left: `${(start / safeClipDuration) * 100}%`, width: `${(width / safeClipDuration) * 100}%` }}
                      onClick={(event) => {
                        event.stopPropagation();
                        setSelectedClip(clip.id);
                      }}
                    >
                      <span>{clip.id}</span>
                      <small>{formatMilliseconds(width)}</small>
                    </button>
                  );
                })}
              </div>
              <div
                className="camera-lane"
                onDoubleClick={(event) => {
                  if ((event.target as HTMLElement).closest(".keyframe-handle")) return;
                  const rect = event.currentTarget.getBoundingClientRect();
                  addKeyframeAt(((event.clientX - rect.left) / rect.width) * duration);
                }}
              >
                <svg className="zoom-curve" viewBox="0 0 1000 100" preserveAspectRatio="none" aria-hidden="true">
                  <polyline points={curve} />
                </svg>
                {keyframes.map((frame, index) => (
                  <button
                    type="button"
                    key={`kf-${index}`}
                    className={`keyframe-handle ${selected === index ? "selected" : ""}`}
                    style={{ left: `${(frame.time_ms / duration) * 100}%`, top: `${zoomToTopPercent(frame.pose.zoom)}%` }}
                    title={`${formatMilliseconds(frame.time_ms)} · ${frame.pose.zoom.toFixed(2)}× · 拖动调整时间和缩放`}
                    disabled={rendering}
                    onPointerDown={(event) => onKeyframePointerDown(event, index)}
                    onClick={(event) => {
                      event.stopPropagation();
                      setSelected(index);
                      const time = keyframesRef.current[index]?.time_ms;
                      if (time != null) seekTo(time);
                    }}
                  />
                ))}
              </div>
              <div className="audio-lane">
                <div className="audio-placeholder">音频占位 · 成片仍保留原片音轨，这条轨道还不能单独剪辑</div>
              </div>
              <div className="playhead" style={{ left: `${Math.min(100, (playheadMs / duration) * 100)}%` }}><span /></div>
            </div>
          </div>
        </div>

        <div className="segment-inspector">
          {selectedFrame ? (
            <>
              <strong>{formatMilliseconds(selectedFrame.time_ms)}</strong>
              <span>{REASON_LABEL[selectedFrame.reason]}</span>
              <span>缩放 {selectedFrame.pose.zoom.toFixed(2)}×</span>
              <span>焦点 X {Math.round(selectedFrame.pose.center.x * 100)}% · Y {Math.round(selectedFrame.pose.center.y * 100)}%</span>
              <span>{cameraDirty ? "镜头已修改，尚未导出" : `${keyframes.length} 个关键帧`}</span>
            </>
          ) : (
            <span>拖动菱形关键帧调整时间与缩放。切换到原片后，可拖动画面中心平移焦点。双击镜头轨可添加关键帧。</span>
          )}
        </div>
        {selectedFrame && (
          <div className="camera-editor">
            <label>
              <span>缩放 <strong>{selectedFrame.pose.zoom.toFixed(2)}×</strong></span>
              <input type="range" min={MIN_ZOOM} max={MAX_ZOOM} step={0.01} value={Math.min(MAX_ZOOM, selectedFrame.pose.zoom)} disabled={rendering} onFocus={snapshotSliders} onBlur={commitSliders} onChange={(event) => updateSelectedPose({ zoom: Number(event.target.value) })} />
            </label>
            <label>
              <span>水平焦点 <strong>{Math.round(selectedFrame.pose.center.x * 100)}%</strong></span>
              <input type="range" min={0} max={1} step={0.01} value={selectedFrame.pose.center.x} disabled={rendering} onFocus={snapshotSliders} onBlur={commitSliders} onChange={(event) => updateSelectedPose({ centerX: Number(event.target.value) })} />
            </label>
            <label>
              <span>垂直焦点 <strong>{Math.round(selectedFrame.pose.center.y * 100)}%</strong></span>
              <input type="range" min={0} max={1} step={0.01} value={selectedFrame.pose.center.y} disabled={rendering} onFocus={snapshotSliders} onBlur={commitSliders} onChange={(event) => updateSelectedPose({ centerY: Number(event.target.value) })} />
            </label>
            <button className="record-button compact" type="button" disabled={!cameraDirty || rendering || keyframes.length < 2} onClick={() => void exportCamera()}>
              {rendering ? "正在重新渲染…" : "导出镜头"}
            </button>
          </div>
        )}
      </section>
    </section>
  );
}
