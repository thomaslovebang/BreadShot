import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { register, unregister } from "@tauri-apps/plugin-global-shortcut";
import { getCurrentWindow } from "@tauri-apps/api/window";
import React, { useEffect, useMemo, useRef, useState } from "react";
import ReactDOM from "react-dom/client";
import "./styles.css";
import { Workbench, type StudioProject } from "./workbench";

type RecorderStatus = {
  isRecording: boolean;
  ffmpegAvailable: boolean;
  outputPath: string | null;
  outputDirectory: string;
  mouseEventsPath: string | null;
  cameraTimelinePath: string | null;
  finalVideoPath: string | null;
  projectPath: string | null;
  directorProfile: DirectorProfile;
};

type DirectorProfile = "clean" | "dynamic" | "soft";

const DEFAULT_SHORTCUT = "F8";
const SINGLE_KEY_PATTERN = /^(?:[A-Z0-9]|F(?:[1-9]|1\d|2[0-4]))$/;

function savedShortcut() {
  const saved = (localStorage.getItem("focusflow.shortcut") || "").toUpperCase();
  return SINGLE_KEY_PATTERN.test(saved) ? saved : DEFAULT_SHORTCUT;
}

function shortcutFromKeyboardEvent(event: KeyboardEvent) {
  if (/^F(?:[1-9]|1\d|2[0-4])$/.test(event.key.toUpperCase())) return event.key.toUpperCase();
  if (/^[A-Z0-9]$/.test(event.key.toUpperCase())) return event.key.toUpperCase();
  return null;
}
const initialStatus: RecorderStatus = {
  isRecording: false,
  ffmpegAvailable: false,
  outputPath: null,
  outputDirectory: "",
  mouseEventsPath: null,
  cameraTimelinePath: null,
  finalVideoPath: null,
  projectPath: null,
  directorProfile: "clean",
};

function formatElapsed(seconds: number) {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const rest = seconds % 60;
  return [hours, minutes, rest].map((value) => value.toString().padStart(2, "0")).join(":");
}

function fileName(path: string) {
  return path.split(/[\\/]/).pop() || path;
}

function App() {
  const [status, setStatus] = useState(initialStatus);
  const [elapsed, setElapsed] = useState(0);
  const [timerRunning, setTimerRunning] = useState(false);
  const [busy, setBusy] = useState(true);
  const [message, setMessage] = useState("正在检查录制环境…");
  const [shortcut, setShortcut] = useState(savedShortcut);
  const [shortcutDialogOpen, setShortcutDialogOpen] = useState(false);
  const [capturedShortcut, setCapturedShortcut] = useState<string | null>(null);
  const [studio, setStudio] = useState<StudioProject | null>(null);
  const statusRef = useRef(status);
  const busyRef = useRef(busy);
  const shortcutRef = useRef(shortcut);
  const shortcutDialogOpenRef = useRef(false);
  const toggleRef = useRef<() => Promise<void>>(async () => undefined);

  useEffect(() => { statusRef.current = status; }, [status]);
  useEffect(() => { busyRef.current = busy; }, [busy]);
  useEffect(() => { shortcutRef.current = shortcut; }, [shortcut]);
  useEffect(() => { shortcutDialogOpenRef.current = shortcutDialogOpen; }, [shortcutDialogOpen]);

  useEffect(() => {
    async function initialize() {
      try {
        let next = await invoke<RecorderStatus>("recorder_status");
        const savedDirectory = localStorage.getItem("focusflow.outputDirectory");
        if (savedDirectory && savedDirectory !== next.outputDirectory) {
          next = await invoke<RecorderStatus>("set_output_directory", { directory: savedDirectory });
        }
        const savedProfile = localStorage.getItem("focusflow.directorProfile") as DirectorProfile | null;
        if (savedProfile && ["clean", "dynamic", "soft"].includes(savedProfile) && savedProfile !== next.directorProfile) {
          await invoke<DirectorProfile>("set_director_profile", { profile: savedProfile });
          next = { ...next, directorProfile: savedProfile };
        }
        setStatus(next);
        setTimerRunning(next.isRecording);
        setMessage(next.ffmpegAvailable ? "准备就绪" : "未找到 FFmpeg，录制功能暂不可用");
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    }
    initialize();
  }, []);

  useEffect(() => {
    let disposed = false;
    async function install() {
      try {
        await unregister(shortcut).catch(() => undefined);
        if (disposed) return;
        await register(shortcut, (event) => {
          if (event.state === "Pressed" && !shortcutDialogOpenRef.current) void toggleRef.current();
        });
      } catch (error) {
        setMessage(`快捷键不可用：${String(error)}`);
      }
    }
    void install();
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
      const next = shortcutFromKeyboardEvent(event);
      if (next) {
        setCapturedShortcut(next);
        setMessage(`已捕获按键 ${next}，点击“更改”后生效`);
      } else {
        setMessage("只支持一个字母、数字或 F1–F24 功能键");
      }
    }
    window.addEventListener("keydown", capture, true);
    return () => window.removeEventListener("keydown", capture, true);
  }, [shortcutDialogOpen]);

  useEffect(() => {
    if (!timerRunning) return;
    const timer = window.setInterval(() => setElapsed((value) => value + 1), 1000);
    return () => window.clearInterval(timer);
  }, [timerRunning]);

  const actionLabel = useMemo(() => {
    if (busy) return "请稍候";
    return status.isRecording ? "停止并保存" : "开始录制";
  }, [busy, status.isRecording]);

  const generatedFiles = [
    status.projectPath && { label: "FocusFlow 项目", path: status.projectPath, project: true },
    status.outputPath && { label: "原始视频", path: status.outputPath },
    status.mouseEventsPath && { label: "鼠标轨迹", path: status.mouseEventsPath },
    status.cameraTimelinePath && { label: "镜头脚本", path: status.cameraTimelinePath },
    status.finalVideoPath && { label: "最终成片", path: status.finalVideoPath, final: true },
  ].filter(Boolean) as Array<{ label: string; path: string; final?: boolean; project?: boolean }>;

  async function restoreAndFocusWindow() {
    const appWindow = getCurrentWindow();
    await appWindow.show();
    await appWindow.unminimize();
    await appWindow.setAlwaysOnTop(true);
    await appWindow.setFocus();
    window.setTimeout(() => void appWindow.setAlwaysOnTop(false), 1200);
  }

  async function toggleRecording() {
    if (busyRef.current) return;
    const appWindow = getCurrentWindow();
    const wasRecording = statusRef.current.isRecording;
    busyRef.current = true;
    setBusy(true);
    setMessage(wasRecording ? "正在分析镜头并渲染最终视频…" : "正在启动录制…");
    if (wasRecording) {
      setTimerRunning(false);
      const processingStatus = { ...statusRef.current, isRecording: false };
      statusRef.current = processingStatus;
      setStatus(processingStatus);
    }
    const command = wasRecording ? "stop_recording" : "start_recording";
    const commandResult = invoke<RecorderStatus>(command).then(
      (value) => ({ value, error: null }),
      (error) => ({ value: null, error }),
    );
    if (wasRecording) {
      await new Promise((resolve) => window.setTimeout(resolve, 1000));
      await restoreAndFocusWindow().catch(() => undefined);
    }
    try {
      const result = await commandResult;
      if (result.error !== null) throw result.error;
      const next = result.value!;
      statusRef.current = next;
      setStatus(next);
      if (next.isRecording) {
        setElapsed(0);
        setTimerRunning(true);
        setMessage(`正在录制 · 再按 ${shortcutRef.current} 停止`);
        await appWindow.hide();
      } else {
        setMessage("最终视频已生成");
        if (next.projectPath) await enterStudio(next.projectPath);
      }
    } catch (error) {
      setMessage(String(error));
      if (wasRecording) {
        await restoreAndFocusWindow();
      }
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  }
  toggleRef.current = toggleRecording;

  async function chooseOutputDirectory() {
    if (status.isRecording || busy) return;
    const selected = await open({
      directory: true,
      multiple: false,
      title: "选择 FocusFlow 视频保存位置",
      defaultPath: status.outputDirectory,
    });
    if (!selected || Array.isArray(selected)) return;
    try {
      const next = await invoke<RecorderStatus>("set_output_directory", { directory: selected });
      setStatus(next);
      localStorage.setItem("focusflow.outputDirectory", selected);
      setMessage("保存位置已更新");
    } catch (error) {
      setMessage(String(error));
    }
  }

  async function chooseProject() {
    if (status.isRecording || busy) return;
    const selected = await open({
      directory: false,
      multiple: false,
      filters: [{ name: "FocusFlow Project", extensions: ["focusflow"] }],
      title: "打开 FocusFlow 项目",
      defaultPath: status.outputDirectory,
    });
    if (!selected || Array.isArray(selected)) return;
    try {
      const next = await invoke<RecorderStatus>("open_project", { path: selected });
      setStatus(next);
      localStorage.setItem("focusflow.directorProfile", next.directorProfile);
      await enterStudio(selected);
      setMessage(`项目已打开：${fileName(selected)}`);
    } catch (error) {
      setMessage(`无法打开项目：${String(error)}`);
    }
  }

  async function enterStudio(projectPath: string) {
    const project = await invoke<StudioProject>("load_studio_project", { path: projectPath });
    setStudio(project);
  }

  async function applyShortcut() {
    const next = capturedShortcut;
    if (!next || status.isRecording) return;
    try {
      await unregister(shortcut).catch(() => undefined);
      await register(next, (event) => {
        if (event.state === "Pressed" && !shortcutDialogOpenRef.current) void toggleRef.current();
      });
      setShortcut(next);
      shortcutRef.current = next;
      localStorage.setItem("focusflow.shortcut", next);
      setMessage(`快捷键已更新为 ${next}`);
      setShortcutDialogOpen(false);
      setCapturedShortcut(null);
    } catch (error) {
      await register(shortcut, (event) => {
        if (event.state === "Pressed" && !shortcutDialogOpenRef.current) void toggleRef.current();
      }).catch(() => undefined);
      setMessage(`快捷键设置失败：${String(error)}`);
    }
  }

  async function changeDirectorProfile(profile: DirectorProfile) {
    if (status.isRecording || busy) return;
    try {
      const next = await invoke<DirectorProfile>("set_director_profile", { profile });
      setStatus((current) => ({ ...current, directorProfile: next }));
      localStorage.setItem("focusflow.directorProfile", next);
      const labels: Record<DirectorProfile, string> = { clean: "Clean", dynamic: "Dynamic", soft: "Soft" };
      setMessage(`导演风格已切换为 ${labels[next]}`);
    } catch (error) {
      setMessage(String(error));
    }
  }

  return (
    <main className="shell">
      <div className="ambient ambient-one" />
      <div className="ambient ambient-two" />

      <section className={`app-frame ${studio ? "workbench-open" : ""}`}>
        <header className="topbar">
          <div className="brand">
            <div className="brand-mark"><span /></div>
            <div><strong>FocusFlow</strong><span>AI Demo Recorder</span></div>
          </div>
          <div className={`status-pill ${status.isRecording ? "live" : busy ? "working" : "ready"}`}>
            <span />{status.isRecording ? "正在录制" : busy ? "处理中" : "本地就绪"}
          </div>
        </header>

        {studio ? (
          <Workbench
            key={`${studio.projectPath}:${studio.previewRevision}`}
            studio={studio}
            message={message}
            onBack={() => setStudio(null)}
            onStudioChange={setStudio}
            onMessage={setMessage}
          />
        ) : (
        <div className="workspace">
          <section className="record-panel">
            <div className="hero-copy">
              <span className="eyebrow">AUTOMATIC CAMERA DIRECTION</span>
              <h1>{status.isRecording ? "专注演示，镜头交给我们。" : "录一次，直接得到专业 Demo。"}</h1>
              <p className="status-message">{message}</p>
            </div>

            <div className={`recorder-console ${status.isRecording ? "is-recording" : ""}`}>
              <div className="timer-block">
                <span className="timer-label">RECORDING TIME</span>
                <div className="timer"><span className="record-dot" />{formatElapsed(elapsed)}</div>
              </div>
              <button className="record-button" type="button" disabled={busy || (!status.ffmpegAvailable && !status.isRecording)} onClick={toggleRecording}>
                <span className="button-icon">{status.isRecording ? "■" : "●"}</span>
                {actionLabel}
              </button>
            </div>

            <div className="feature-strip">
              <span><i>60</i> FPS</span>
              <span><i>{status.directorProfile === "dynamic" ? "1.45×" : status.directorProfile === "soft" ? "1.25×" : "1.34×"}</i> Smart Zoom</span>
              <span><i>100%</i> Local</span>
            </div>
          </section>

          <aside className="side-panel">
            <section className="panel-card settings-card">
              <div className="card-heading"><div><span>01</span><strong>录制设置</strong></div><small>录制中自动锁定</small></div>
              <div className="setting-row">
                <div><strong>保存位置</strong><span title={status.outputDirectory}>{status.outputDirectory || "尚未设置"}</span></div>
                <button className="secondary-button" type="button" disabled={status.isRecording || busy} onClick={chooseOutputDirectory}>更改</button>
              </div>
              <div className="setting-row">
                <div><strong>导演风格</strong><span>决定缩放强度与镜头移动节奏</span></div>
                <select className="profile-select" value={status.directorProfile} disabled={status.isRecording || busy} onChange={(event) => void changeDirectorProfile(event.target.value as DirectorProfile)}>
                  <option value="clean">Clean</option>
                  <option value="dynamic">Dynamic</option>
                  <option value="soft">Soft</option>
                </select>
              </div>
              <div className="setting-row">
                <div><strong>全局快捷键</strong><span>开始与停止使用同一按键</span></div>
                <div className="shortcut-editor">
                  <kbd>{shortcut}</kbd>
                  <button className="secondary-button" type="button" disabled={status.isRecording} onClick={() => { setCapturedShortcut(null); setShortcutDialogOpen(true); }}>更改</button>
                </div>
              </div>
            </section>

            <section className="panel-card output-card">
              <div className="card-heading"><div><span>02</span><strong>最近生成</strong></div><small>{generatedFiles.length ? `${generatedFiles.length} 个文件` : "等待首次录制"}</small></div>
              <button className="secondary-button" type="button" disabled={status.isRecording || busy} onClick={chooseProject}>打开 .focusflow 项目</button>
              <div className="file-list">
                {generatedFiles.length === 0 ? (
                  <div className="empty-output"><span>◇</span><p>完成录制后，原片、轨迹与成片会显示在这里。</p></div>
                ) : generatedFiles.map((file) => (
                  <div className={`file-row ${file.final ? "final" : ""}`} key={file.label} title={file.path}>
                    <span className="file-icon">{file.project ? "FF" : file.final ? "▶" : "·"}</span>
                    <div><strong>{file.label}</strong><small>{fileName(file.path)}</small></div>
                    {file.final && <em>READY</em>}
                  </div>
                ))}
              </div>
            </section>
          </aside>
        </div>
        )}

        <footer className="app-footer"><span>视频全程在本机处理</span><span>Windows · H.264 MP4 · 无水印</span></footer>
      </section>

      {shortcutDialogOpen && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => setShortcutDialogOpen(false)}>
          <section className="shortcut-modal" role="dialog" aria-modal="true" aria-labelledby="shortcut-title" onMouseDown={(event) => event.stopPropagation()}>
            <span className="eyebrow">快捷键设置</span>
            <h2 id="shortcut-title">请直接点击想设置的快捷键</h2>
            <p>只能设置一个按键，例如 F8、G 或 1，不区分大小写。</p>
            <div className={`captured-key ${capturedShortcut ? "has-key" : ""}`}>
              {capturedShortcut || "等待按键…"}
            </div>
            <div className="modal-actions">
              <button className="secondary-button" type="button" onClick={() => setShortcutDialogOpen(false)}>取消</button>
              <button className="record-button compact" type="button" disabled={!capturedShortcut} onClick={applyShortcut}>更改</button>
            </div>
          </section>
        </div>
      )}
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
