// Camera keyframe edits for the Phase 9 workbench.
// Keep keyframesFromTimeline / samplePose aligned with crates/editor_core.

export type CameraEasing = "linear" | "ease_in_out_cubic";
export type FocusReason = "full_frame" | "dwell" | "click" | "transition";

export type CameraPose = {
  center: { x: number; y: number };
  zoom: number;
};

export type CameraSegment = {
  start_ms: number;
  end_ms: number;
  from: CameraPose;
  to: CameraPose;
  easing: CameraEasing;
  reason: FocusReason;
};

export type CameraTimeline = {
  schema_version: number;
  duration_ms: number;
  segments: CameraSegment[];
};

export type CameraKeyframe = {
  time_ms: number;
  pose: CameraPose;
  easing: CameraEasing;
  reason: FocusReason;
};

export const MIN_ZOOM = 1;
export const MAX_ZOOM = 2.2;

const FULL_POSE: CameraPose = { center: { x: 0.5, y: 0.5 }, zoom: 1 };

export function cloneKeyframes(frames: CameraKeyframe[]): CameraKeyframe[] {
  return frames.map((frame) => ({
    ...frame,
    pose: { zoom: frame.pose.zoom, center: { ...frame.pose.center } },
  }));
}

export function keyframesFromTimeline(timeline: CameraTimeline): CameraKeyframe[] {
  if (timeline.segments.length === 0) {
    if (timeline.duration_ms === 0) return [fullKeyframe(0, FULL_POSE)];
    return [fullKeyframe(0, FULL_POSE), fullKeyframe(timeline.duration_ms, FULL_POSE)];
  }

  const frames: CameraKeyframe[] = [];
  const first = timeline.segments[0];
  if (first.start_ms > 0) frames.push(fullKeyframe(0, first.from));
  frames.push({
    time_ms: first.start_ms,
    pose: clonePose(first.from),
    easing: "linear",
    reason: "full_frame",
  });
  for (const segment of timeline.segments) {
    const last = frames.at(-1);
    if (last && segment.end_ms <= last.time_ms) continue;
    frames.push({
      time_ms: segment.end_ms,
      pose: clonePose(segment.to),
      easing: segment.easing,
      reason: segment.reason,
    });
  }
  const last = frames.at(-1);
  if (last && last.time_ms < timeline.duration_ms) {
    frames.push(fullKeyframe(timeline.duration_ms, last.pose));
  }
  return frames;
}

export function samplePose(frames: CameraKeyframe[], timeMs: number): CameraPose {
  const first = frames[0];
  if (!first) return { ...FULL_POSE, center: { ...FULL_POSE.center } };
  if (timeMs <= first.time_ms) return clonePose(first.pose);
  const last = frames.at(-1)!;
  if (timeMs >= last.time_ms) return clonePose(last.pose);
  const index = frames.findIndex((frame) => frame.time_ms > timeMs);
  const left = frames[index - 1];
  const right = frames[index];
  if (!left || !right) return clonePose(last.pose);
  const span = right.time_ms - left.time_ms;
  if (span <= 0) return clonePose(right.pose);
  const progress = (timeMs - left.time_ms) / span;
  return {
    center: {
      x: lerp(left.pose.center.x, right.pose.center.x, progress),
      y: lerp(left.pose.center.y, right.pose.center.y, progress),
    },
    zoom: lerp(left.pose.zoom, right.pose.zoom, progress),
  };
}

export function moveKeyframe(
  frames: CameraKeyframe[],
  index: number,
  timeMs: number,
  zoom: number,
): CameraKeyframe[] | null {
  if (index < 0 || index >= frames.length) return null;
  const next = cloneKeyframes(frames);
  const frame = next[index];
  let time = Math.round(timeMs);
  if (index === 0) time = 0;
  else if (index === next.length - 1) time = frame.time_ms;
  else {
    const min = next[index - 1].time_ms + 1;
    const max = next[index + 1].time_ms - 1;
    if (max < min) return next;
    time = Math.min(max, Math.max(min, time));
  }
  const clampedZoom = clampZoom(zoom);
  frame.time_ms = time;
  frame.pose = {
    zoom: clampedZoom,
    center: {
      x: clampCenter(frame.pose.center.x, clampedZoom),
      y: clampCenter(frame.pose.center.y, clampedZoom),
    },
  };
  return next;
}

export function setKeyframePose(
  frames: CameraKeyframe[],
  index: number,
  pose: CameraPose,
): CameraKeyframe[] | null {
  if (index < 0 || index >= frames.length) return null;
  const next = cloneKeyframes(frames);
  const zoom = clampZoom(pose.zoom);
  next[index].pose = {
    zoom,
    center: {
      x: clampCenter(pose.center.x, zoom),
      y: clampCenter(pose.center.y, zoom),
    },
  };
  return next;
}

export function insertKeyframe(frames: CameraKeyframe[], timeMs: number): { frames: CameraKeyframe[]; index: number } | null {
  if (frames.length < 2) return null;
  const last = frames.at(-1)!;
  const time = Math.round(timeMs);
  if (time <= frames[0].time_ms || time >= last.time_ms) return null;
  if (frames.some((frame) => frame.time_ms === time)) return null;
  const pose = clampPose(samplePose(frames, time));
  const index = frames.findIndex((frame) => frame.time_ms > time);
  const next = cloneKeyframes(frames);
  next.splice(index, 0, {
    time_ms: time,
    pose,
    easing: "ease_in_out_cubic",
    reason: "transition",
  });
  return { frames: next, index };
}

export function removeKeyframe(frames: CameraKeyframe[], index: number): CameraKeyframe[] | null {
  if (frames.length <= 2 || index <= 0 || index >= frames.length - 1) return null;
  const next = cloneKeyframes(frames);
  next.splice(index, 1);
  return next;
}

export function resetKeyframePose(
  frames: CameraKeyframe[],
  index: number,
  baseline: CameraKeyframe[],
): CameraKeyframe[] | null {
  if (index < 0 || index >= frames.length || baseline.length === 0) return null;
  const pose = clampPose(samplePose(baseline, frames[index].time_ms));
  return setKeyframePose(frames, index, pose);
}

export function clampZoom(zoom: number): number {
  if (!Number.isFinite(zoom)) return MIN_ZOOM;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

export function clampCenter(value: number, zoom: number): number {
  const half = 0.5 / Math.max(zoom, MIN_ZOOM);
  if (!Number.isFinite(value)) return 0.5;
  return Math.min(1 - half, Math.max(half, value));
}

export function zoomToTopPercent(zoom: number): number {
  const span = MAX_ZOOM - MIN_ZOOM;
  const ratio = Math.min(1, Math.max(0, (zoom - MIN_ZOOM) / span));
  return 88 - ratio * 76;
}

export function sameKeyframes(left: CameraKeyframe[], right: CameraKeyframe[]): boolean {
  if (left.length !== right.length) return false;
  return left.every((frame, index) => {
    const other = right[index];
    return (
      frame.time_ms === other.time_ms &&
      Math.abs(frame.pose.zoom - other.pose.zoom) < 0.000_1 &&
      Math.abs(frame.pose.center.x - other.pose.center.x) < 0.000_1 &&
      Math.abs(frame.pose.center.y - other.pose.center.y) < 0.000_1
    );
  });
}

function fullKeyframe(timeMs: number, pose: CameraPose): CameraKeyframe {
  return { time_ms: timeMs, pose: clonePose(pose), easing: "linear", reason: "full_frame" };
}

function clonePose(pose: CameraPose): CameraPose {
  return { zoom: pose.zoom, center: { x: pose.center.x, y: pose.center.y } };
}

function clampPose(pose: CameraPose): CameraPose {
  const zoom = clampZoom(pose.zoom);
  return {
    zoom,
    center: {
      x: clampCenter(pose.center.x, zoom),
      y: clampCenter(pose.center.y, zoom),
    },
  };
}

function lerp(from: number, to: number, progress: number): number {
  return from + (to - from) * progress;
}

