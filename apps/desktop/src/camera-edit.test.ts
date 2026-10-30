import assert from "node:assert/strict";
import { test } from "node:test";
import {
  insertKeyframe,
  keyframesFromTimeline,
  moveKeyframe,
  removeKeyframe,
  resetKeyframePose,
  samplePose,
  type CameraTimeline,
} from "./camera-edit.ts";

const timeline: CameraTimeline = {
  schema_version: 1,
  duration_ms: 2_000,
  segments: [
    {
      start_ms: 0,
      end_ms: 800,
      from: { center: { x: 0.5, y: 0.5 }, zoom: 1 },
      to: { center: { x: 0.4, y: 0.6 }, zoom: 1.34 },
      easing: "ease_in_out_cubic",
      reason: "click",
    },
    {
      start_ms: 800,
      end_ms: 2_000,
      from: { center: { x: 0.4, y: 0.6 }, zoom: 1.34 },
      to: { center: { x: 0.5, y: 0.5 }, zoom: 1 },
      easing: "ease_in_out_cubic",
      reason: "full_frame",
    },
  ],
};

test("keyframes follow segment boundaries", () => {
  const frames = keyframesFromTimeline(timeline);
  assert.deepEqual(
    frames.map((frame) => frame.time_ms),
    [0, 800, 2_000],
  );
  assert.equal(frames[1].pose.zoom, 1.34);
  assert.equal(frames[1].reason, "click");
});

test("empty automatic timeline becomes a full-frame hold", () => {
  const frames = keyframesFromTimeline({ schema_version: 1, duration_ms: 4_000, segments: [] });
  assert.equal(frames.length, 2);
  assert.equal(frames[0].pose.zoom, 1);
  assert.equal(frames[1].time_ms, 4_000);
});

test("sample, move, insert, remove, and reset stay inside the path", () => {
  const frames = keyframesFromTimeline(timeline);
  const sampled = samplePose(frames, 400);
  assert.ok(sampled.zoom > 1 && sampled.zoom < 1.34);

  const moved = moveKeyframe(frames, 1, 200, 1.8);
  assert.ok(moved);
  assert.equal(moved[1].time_ms, 200);
  assert.equal(moved[1].pose.zoom, 1.8);
  assert.equal(moveKeyframe(frames, 0, 500, 1.2)?.[0].time_ms, 0);
  assert.equal(moveKeyframe(frames, 2, 100, 1.5)?.[2].time_ms, 2_000);

  const inserted = insertKeyframe(frames, 1_000);
  assert.ok(inserted);
  assert.equal(inserted.frames[inserted.index].reason, "transition");
  assert.equal(removeKeyframe(inserted.frames, inserted.index)?.length, frames.length);
  assert.equal(removeKeyframe(frames, 0), null);

  const reset = resetKeyframePose(moved, 1, frames);
  assert.ok(reset);
  assert.equal(reset[1].pose.zoom, samplePose(frames, reset[1].time_ms).zoom);
});
