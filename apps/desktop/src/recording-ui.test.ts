import assert from "node:assert/strict";
import { test } from "node:test";
import {
  choiceFromStorage,
  choiceToStorage,
  hotkeyRegistrationMessage,
  isTransformCallbackFailure,
  startDisplayArgs,
  tauriCallbackBridgeReady,
  type CaptureDisplay,
} from "./recording-ui.ts";

const displays: CaptureDisplay[] = [
  { index: 0, name: "\\\\.\\DISPLAY1", label: "显示器 1：DISPLAY1（主屏幕，1920×1080）", x: 0, y: 0, width: 1920, height: 1080, primary: true },
  { index: 1, name: "\\\\.\\DISPLAY2", label: "显示器 2：DISPLAY2（扩展屏幕，2560×1440）", x: 1920, y: 0, width: 2560, height: 1440, primary: false },
];

test("transformCallback failures become a clickable Chinese message", () => {
  const error = new TypeError("Cannot read properties of undefined (reading 'transformCallback')");
  assert.equal(isTransformCallbackFailure(error), true);
  const message = hotkeyRegistrationMessage("F8", error);
  assert.match(message, /快捷键 F8/);
  assert.match(message, /所有按钮仍可点击/);
  assert.equal(message.includes("transformCallback"), false);
});

test("occupied hotkeys keep a Chinese conflict message", () => {
  const message = hotkeyRegistrationMessage("F8", "HotKey already registered");
  assert.match(message, /可能已被其他程序占用/);
  assert.match(message, /请更换快捷键/);
});

test("a backend Chinese failure is shown as-is", () => {
  const backend = "快捷键 G 无法注册，可能已被其他程序占用。所有按钮仍可点击，请更换快捷键。";
  assert.equal(hotkeyRegistrationMessage("G", `Error: ${backend}`), backend);
});

test("the callback bridge is ready only when both hooks exist", () => {
  assert.equal(tauriCallbackBridgeReady(undefined), false);
  assert.equal(tauriCallbackBridgeReady({}), false);
  assert.equal(tauriCallbackBridgeReady({ transformCallback: () => 1, invoke: () => 1 }), true);
});

test("display choice falls back to the primary monitor and round-trips", () => {
  assert.deepEqual(choiceFromStorage(null, displays), { index: 0, name: "\\\\.\\DISPLAY1" });
  assert.equal(choiceFromStorage("all", displays), "all");
  const stored = choiceToStorage({ index: 1, name: "\\\\.\\DISPLAY2" });
  assert.deepEqual(choiceFromStorage(stored, displays), { index: 1, name: "\\\\.\\DISPLAY2" });
  assert.deepEqual(startDisplayArgs("all"), { displayIndex: null, displayName: null });
  assert.deepEqual(startDisplayArgs({ index: 1, name: "\\\\.\\DISPLAY2" }), {
    displayIndex: 1,
    displayName: "\\\\.\\DISPLAY2",
  });
});
