import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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

type ProcessingProgress = {
  stage: "saving" | "analyzing" | "rendering" | "complete";
  progress: number;
  message: string;
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

function readableError(error: unknown) {
  return String(error).replace(/^Error:\s*/i, "");
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
  const [processing, setProcessing] = useState<ProcessingProgress | null>(null);
  const [failure, setFailure] = useState<string | null>(null);
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
    let unlisten: (() => void) | undefined;
    void listen<ProcessingProgress>("processing-progress", (event) => {
      setProcessing(event.payload);
      setMessage(event.payload.message);
    }).then((dispose) => { unlisten = dispose; });
    return () => unlisten?.();
  }, []);

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
    setFailure(null);
    setProcessing(wasRecording ? { stage: "saving", progress: 5, message: "正在结束录制…" } : null);
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
        setProcessing(null);
        setMessage("自动运镜视频已生成");
      }
    } catch (error) {
      const description = readableError(error);
      setFailure(description);
      setProcessing(null);
      setMessage(wasRecording ? "成片生成失败，但原始录制通常仍已保存" : "录制未能开始");
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

  async function openFinalVideo() {
    try {
      await invoke("open_final_video");
    } catch (error) {
      setFailure(readableError(error));
    }
  }

  async function openSourceVideo() {
    try {
      await invoke("open_source_video");
    } catch (error) {
      setFailure(readableError(error));
    }
  }

  async function revealOutput() {
    try {
      await invoke("reveal_output");
    } catch (error) {
      setFailure(readableError(error));
    }
  }

  const selectedDisplay = displays.find((display) => display.id === selectedDisplayId);

  return (
    <main className="shell">
      <section className="app-frame">
        <header className="topbar">
          <div className="brand">
            <span className="brand-mark"><i /></span>
            <div><strong>FocusFlow</strong><small>Automatic camera recorder</small></div>
          </div>
          <span className="alpha-badge">ALPHA</span>
          <div className={`status-pill ${status.isRecording ? "live" : busy ? "working" : "ready"}`}><span />{status.isRecording ? "正在录制" : busy ? "正在处理" : "可以开始"}</div>
        </header>

        <div className="workspace">
          <section className="hero">
            <div className="hero-ambient one" /><div className="hero-ambient two" />
            <p className="eyebrow"><i /> AUTOMATIC CAMERA RECORDER</p>
            <h1>{status.isRecording ? <>正在录制，<em>专注演示就好。</em></> : <>按下录制，<em>镜头交给 FocusFlow。</em></>}</h1>
            <p className="hero-subtitle">录制你的操作，自动生成平滑缩放与跟随镜头。</p>
            <p className="message"><span />{message}</p>
            <div className={`record-console ${status.isRecording ? "is-recording" : ""}`}>
              <div className="capture-main">
                <div className="timer-block"><span className="timer-label">{status.isRecording ? "RECORDING" : "DURATION"}</span><strong className="timer"><i />{formatElapsed(elapsed)}</strong></div>
                <button type="button" className="primary-button record-action" disabled={busy || (!status.ffmpegAvailable && !status.isRecording)} onClick={() => void toggleRecording()}><span className="record-glyph">{status.isRecording ? "■" : ""}</span>{actionLabel}</button>
              </div>
              <div className="capture-foot"><span><kbd>{shortcut}</kbd> 随时开始或结束</span><span>{selectedDisplay ? selectedDisplay.label : "正在检测显示器"}</span></div>
            </div>
            <div className="outcome-slot">
              {busy && processing && (
                <section className="processing-card" aria-live="polite">
                  <div className="processing-copy"><span className="spinner" /><div><strong>{processing.message}</strong><span>正在本机完成处理，请保持 FocusFlow 运行</span></div><b>{processing.progress}%</b></div>
                  <div className="progress-track"><i style={{ width: `${processing.progress}%` }} /></div>
                </section>
              )}
              {!busy && failure && (
                <section className="failure-card" role="alert">
                  <div><strong>这次成片没有顺利生成</strong><span>{failure}</span></div>
                  <button type="button" className="secondary-button" onClick={() => void revealOutput()}>查看已保存文件</button>
                </section>
              )}
              {!busy && !failure && status.finalVideoPath && (
                <section className="success-card">
                  <div className="success-icon">✓</div>
                  <div className="success-copy"><strong>成片已准备好</strong><span title={status.finalVideoPath}>{fileName(status.finalVideoPath)}</span></div>
                  <div className="result-actions"><button type="button" className="secondary-button" onClick={() => void revealOutput()}>打开文件夹</button><button type="button" className="primary-button compact" onClick={() => void openFinalVideo()}>▶ 播放最终视频</button><button type="button" className="secondary-button" onClick={() => void openSourceVideo()}>播放原始视频</button></div>
                </section>
              )}
              {!busy && !failure && !status.finalVideoPath && <div className="promise-row"><span>∞ 不限时长</span><span>◇ 无水印 MP4</span><span>⌁ 自动缩放跟随</span></div>}
            </div>
          </section>

          <aside className="settings">
            <div className="settings-heading"><div><span>录制设置</span><small>确认以下三项，即可开始</small></div><b>3</b></div>
            <div className="setting-row">
              <span className="setting-index">01</span><div className="setting-copy"><strong>保存位置</strong><span title={status.outputDirectory}>{status.outputDirectory || "尚未设置"}</span></div>
              <button type="button" className="secondary-button" disabled={status.isRecording || busy} onClick={() => void chooseOutputDirectory()}>选择</button>
            </div>
            <div className="setting-row">
              <span className="setting-index">02</span><div className="setting-copy"><strong>录制显示器</strong><span>只捕获选择的这一块屏幕</span></div>
              <select className="display-select" value={selectedDisplayId} disabled={status.isRecording || busy} onChange={(event) => changeDisplay(event.target.value)}>
                {displays.map((display) => <option key={display.id} value={display.id}>{display.label}</option>)}
              </select>
            </div>
            <div className="setting-row">
              <span className="setting-index">03</span><div className="setting-copy"><strong>全局快捷键</strong><span>同一个按键开始与结束</span></div>
              <div className="shortcut-control"><kbd>{shortcut}</kbd><button type="button" className="secondary-button" disabled={status.isRecording || busy} onClick={() => { setCapturedShortcut(null); setShortcutDialogOpen(true); }}>更改</button></div>
            </div>
            <div className="privacy-note"><span>⌾</span><div><strong>本机私密处理</strong><small>画面与鼠标数据不会上传</small></div></div>
          </aside>
        </div>

        <footer><span>FocusFlow Alpha · Built for Windows</span><div><span>LOCAL PROCESSING</span><i /> <span>H.264 MP4</span><i /> <span>NO WATERMARK</span></div></footer>
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
