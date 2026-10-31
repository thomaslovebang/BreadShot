import { invoke } from "@tauri-apps/api/core";
import { availableMonitors, getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import React, { useEffect, useMemo, useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";

type RecorderStatus = {
  isRecording: boolean;
  ffmpegAvailable: boolean;
  outputPath: string | null;
  outputDirectory: string;
  finalVideoPath: string | null;
};

type CaptureDisplay = {
  id: string;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
};

const DEFAULT_SHORTCUT = "F8";
const SINGLE_KEY_PATTERN = /^(?:[A-Z0-9]|F(?:[1-9]|1\d|2[0-4]))$/;
const initialStatus: RecorderStatus = { isRecording: false, ffmpegAvailable: false, outputPath: null, outputDirectory: "", finalVideoPath: null };

function readSavedShortcut() {
  const value = (localStorage.getItem("focusflow.shortcut") || "").toUpperCase();
  return SINGLE_KEY_PATTERN.test(value) ? value : DEFAULT_SHORTCUT;
}

function shortcutFromEvent(event: KeyboardEvent) {
  const key = event.key.toUpperCase();
  return SINGLE_KEY_PATTERN.test(key) ? key : null;
}

function formatElapsed(seconds: number) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  return [hours, minutes, rest].map((value) => String(value).padStart(2, "0")).join(":");
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

function App() {
  const [status, setStatus] = useState(initialStatus);
  const [elapsed, setElapsed] = useState(0);
  const [busy, setBusy] = useState(true);
  const [message, setMessage] = useState("正在检查录制环境…");
  const [shortcut, setShortcut] = useState(readSavedShortcut);
  const [displays, setDisplays] = useState<CaptureDisplay[]>([]);
  const [selectedDisplayId, setSelectedDisplayId] = useState("");
  const [shortcutDialogOpen, setShortcutDialogOpen] = useState(false);
  const [capturedShortcut, setCapturedShortcut] = useState<string | null>(null);
  const statusRef = useRef(status);
  const busyRef = useRef(busy);
  const shortcutRef = useRef(shortcut);
  const shortcutDialogRef = useRef(shortcutDialogOpen);
  const toggleRef = useRef<() => Promise<void>>(async () => undefined);

  useEffect(() => { statusRef.current = status; }, [status]);
  useEffect(() => { busyRef.current = busy; }, [busy]);
  useEffect(() => { shortcutRef.current = shortcut; }, [shortcut]);
  useEffect(() => { shortcutDialogRef.current = shortcutDialogOpen; }, [shortcutDialogOpen]);

  useEffect(() => {
    async function initialize() {
      try {
        const monitorList = (await availableMonitors()).map((monitor, index) => {
          const { x, y } = monitor.position;
          const { width, height } = monitor.size;
          return {
            id: `${x}:${y}:${width}:${height}`,
            label: `${monitor.name || `显示器 ${index + 1}`} · ${width} × ${height}`,
            x,
            y,
            width,
            height,
          };
        });
        if (monitorList.length === 0) throw new Error("没有检测到可录制的显示器");
        const savedDisplayId = localStorage.getItem("focusflow.captureDisplay");
        const initialDisplay = monitorList.find((monitor) => monitor.id === savedDisplayId) || monitorList[0];
        setDisplays(monitorList);
        setSelectedDisplayId(initialDisplay.id);

        let next = await invoke<RecorderStatus>("recorder_status");
        const savedDirectory = localStorage.getItem("focusflow.outputDirectory");
        if (savedDirectory && savedDirectory !== next.outputDirectory) {
          next = await invoke<RecorderStatus>("set_output_directory", { directory: savedDirectory });
        }
        setStatus(next);
        setMessage(next.ffmpegAvailable ? "准备就绪" : "未找到 FFmpeg，暂时无法录制");
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    }
    void initialize();
  }, []);

  useEffect(() => {
    let disposed = false;
    async function installShortcut() {
      try {
        await unregister(shortcut).catch(() => undefined);
        if (disposed) return;
        await register(shortcut, (event) => {
          if (event.state === "Pressed" && !shortcutDialogRef.current) void toggleRef.current();
        });
      } catch (error) {
        setMessage(`快捷键不可用：${String(error)}`);
      }
    }
    void installShortcut();
    return () => {
      disposed = true;
      void unregister(shortcut).catch(() => undefined);
    };
  }, [shortcut]);

  useEffect(() => {
    if (!shortcutDialogOpen) return;
    function capture(event: KeyboardEvent) {
      event.preventDefault();
      event.stopPropagation();
      const next = shortcutFromEvent(event);
      if (next) {
        setCapturedShortcut(next);
        setMessage(`已选择 ${next}`);
      } else {
        setMessage("请选择一个字母、数字或 F1–F24 功能键");
      }
    }
    window.addEventListener("keydown", capture, true);
    return () => window.removeEventListener("keydown", capture, true);
  }, [shortcutDialogOpen]);

  useEffect(() => {
    if (!status.isRecording) return;
    const timer = window.setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [status.isRecording]);

  const actionLabel = useMemo(() => {
    if (busy) return status.isRecording ? "正在生成视频…" : "请稍候…";
    return status.isRecording ? "结束录制" : "开始录制";
  }, [busy, status.isRecording]);

  async function restoreWindow() {
    const appWindow = getCurrentWindow();
    await appWindow.show();
    await appWindow.unminimize();
    await appWindow.setAlwaysOnTop(true);
    await appWindow.setFocus();
    window.setTimeout(() => void appWindow.setAlwaysOnTop(false), 1200);
  }

  async function toggleRecording() {
    if (busyRef.current) return;
    const wasRecording = statusRef.current.isRecording;
    const appWindow = getCurrentWindow();
    busyRef.current = true;
    setBusy(true);
    setMessage(wasRecording ? "正在根据鼠标动作生成自动运镜视频…" : "正在启动录制…");

    if (wasRecording) {
      const processing = { ...statusRef.current, isRecording: false };
      statusRef.current = processing;
      setStatus(processing);
    }

    try {
      let resultPromise: Promise<RecorderStatus>;
      if (wasRecording) {
        resultPromise = invoke<RecorderStatus>("stop_recording");
        await new Promise((resolve) => window.setTimeout(resolve, 800));
        await restoreWindow().catch(() => undefined);
      } else {
        const selectedDisplay = displays.find((display) => display.id === selectedDisplayId);
        if (!selectedDisplay) throw new Error("请先选择要录制的显示器");
        await appWindow.hide();
        await new Promise((resolve) => window.setTimeout(resolve, 700));
        resultPromise = invoke<RecorderStatus>("start_recording", {
          captureArea: {
            x: selectedDisplay.x,
            y: selectedDisplay.y,
            width: selectedDisplay.width,
            height: selectedDisplay.height,
          },
        });
      }
      const next = await resultPromise;
      statusRef.current = next;
      setStatus(next);
      if (next.isRecording) {
        setElapsed(0);
        setMessage(`正在录制 · 按 ${shortcutRef.current} 结束`);
      } else {
        setMessage("自动运镜视频已生成");
      }
    } catch (error) {
      setMessage(String(error));
      await restoreWindow().catch(() => undefined);
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }
  toggleRef.current = toggleRecording;

  async function chooseOutputDirectory() {
    if (status.isRecording || busy) return;
    const selected = await open({ directory: true, multiple: false, title: "选择视频保存位置", defaultPath: status.outputDirectory });
    if (!selected || Array.isArray(selected)) return;
    try {
      const next = await invoke<RecorderStatus>("set_output_directory", { directory: selected });
      statusRef.current = next;
      setStatus(next);
      localStorage.setItem("focusflow.outputDirectory", selected);
      setMessage("保存位置已更新");
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function applyShortcut() {
    if (!capturedShortcut || status.isRecording) return;
    const previous = shortcut;
    try {
      await unregister(previous).catch(() => undefined);
      await register(capturedShortcut, (event) => {
        if (event.state === "Pressed" && !shortcutDialogRef.current) void toggleRef.current();
      });
      setShortcut(capturedShortcut);
      shortcutRef.current = capturedShortcut;
      localStorage.setItem("focusflow.shortcut", capturedShortcut);
      setMessage(`开始与结束快捷键已设为 ${capturedShortcut}`);
      setShortcutDialogOpen(false);
      setCapturedShortcut(null);
    } catch (error) {
      await register(previous, (event) => {
        if (event.state === "Pressed" && !shortcutDialogRef.current) void toggleRef.current();
      }).catch(() => undefined);
      setMessage(`快捷键设置失败：${String(error)}`);
    }
  }

  function changeDisplay(id: string) {
    setSelectedDisplayId(id);
    localStorage.setItem("focusflow.captureDisplay", id);
    const display = displays.find((item) => item.id === id);
    if (display) setMessage(`将录制 ${display.label}`);
  }

  return (
    <main className="shell">
      <section className="app-frame">
        <header className="topbar">
          <div className="brand"><span className="brand-mark" /><strong>FocusFlow</strong></div>
          <div className={`status-pill ${status.isRecording ? "live" : busy ? "working" : "ready"}`}><span />{status.isRecording ? "录制中" : busy ? "处理中" : "就绪"}</div>
        </header>

        <div className="workspace">
          <section className="hero">
            <p className="eyebrow">免费 · 无水印 · 不限制录制时长</p>
            <h1>{status.isRecording ? "继续你的演示。" : "录完，自动得到会运镜的视频。"}</h1>
            <p className="message">{message}</p>
            <div className={`record-console ${status.isRecording ? "is-recording" : ""}`}>
              <div><span className="timer-label">录制时间</span><strong className="timer"><i />{formatElapsed(elapsed)}</strong></div>
              <button type="button" className="primary-button" disabled={busy || (!status.ffmpegAvailable && !status.isRecording)} onClick={() => void toggleRecording()}><span>{status.isRecording ? "■" : "●"}</span>{actionLabel}</button>
            </div>
            <div className="promise-row"><span>不限时长</span><span>无水印 MP4</span><span>鼠标驱动自动缩放</span></div>
          </section>

          <aside className="settings">
            <div className="settings-heading"><span>录制设置</span><small>只需设置一次</small></div>
            <div className="setting-row">
              <div><strong>视频保存位置</strong><span title={status.outputDirectory}>{status.outputDirectory || "尚未设置"}</span></div>
              <button type="button" className="secondary-button" disabled={status.isRecording || busy} onClick={() => void chooseOutputDirectory()}>选择</button>
            </div>
            <div className="setting-row">
              <div><strong>录制显示器</strong><span>只录制选择的这一块屏幕</span></div>
              <select className="display-select" value={selectedDisplayId} disabled={status.isRecording || busy} onChange={(event) => changeDisplay(event.target.value)}>
                {displays.map((display) => <option key={display.id} value={display.id}>{display.label}</option>)}
              </select>
            </div>
            <div className="setting-row">
              <div><strong>开始 / 结束快捷键</strong><span>同一个按键切换录制状态</span></div>
              <div className="shortcut-control"><kbd>{shortcut}</kbd><button type="button" className="secondary-button" disabled={status.isRecording || busy} onClick={() => { setCapturedShortcut(null); setShortcutDialogOpen(true); }}>更改</button></div>
            </div>
            <div className="setting-row output-row">
              <div><strong>最近生成的视频</strong><span title={status.finalVideoPath || ""}>{status.finalVideoPath ? fileName(status.finalVideoPath) : "完成录制后显示在这里"}</span></div>
              <span className={status.finalVideoPath ? "result ready" : "result"}>{status.finalVideoPath ? "已完成" : "等待录制"}</span>
            </div>
          </aside>
        </div>

        <footer><span>视频全程在本机处理</span><span>Windows · H.264 MP4</span></footer>
      </section>

      {shortcutDialogOpen && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => setShortcutDialogOpen(false)}>
          <section className="shortcut-modal" role="dialog" aria-modal="true" aria-labelledby="shortcut-title" onMouseDown={(event) => event.stopPropagation()}>
            <p className="eyebrow">快捷键设置</p><h2 id="shortcut-title">按下一个键</h2><p>它将同时用于开始和结束录制。</p>
            <div className={`captured-key ${capturedShortcut ? "has-key" : ""}`}>{capturedShortcut || "等待按键…"}</div>
            <div className="modal-actions"><button type="button" className="secondary-button" onClick={() => setShortcutDialogOpen(false)}>取消</button><button type="button" className="primary-button compact" disabled={!capturedShortcut} onClick={() => void applyShortcut()}>保存</button></div>
          </section>
        </div>
      )}
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
