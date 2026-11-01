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

type AppPage = "record" | "settings" | "language" | "update";
type AppLanguage = "system" | "zh" | "en" | "ja" | "fr" | "de" | "es" | "ar";
type ResolvedLanguage = Exclude<AppLanguage, "system">;

const DEFAULT_SHORTCUT = "F8";
const APP_VERSION = "0.2.1";
const SINGLE_KEY_PATTERN = /^(?:[A-Z0-9]|F(?:[1-9]|1\d|2[0-4]))$/;
const initialStatus: RecorderStatus = { isRecording: false, ffmpegAvailable: false, outputPath: null, outputDirectory: "", finalVideoPath: null };

const translations = {
  zh: { record: "录制", settings: "录制设置", language: "语言", update: "更新", ready: "可以开始", recording: "正在录制", working: "正在处理", eyebrow: "自动运镜录屏", idleTitle: "按下录制，", idleAccent: "镜头交给 FocusFlow。", liveTitle: "正在录制，", liveAccent: "专注演示就好。", subtitle: "录制你的操作，自动生成平滑缩放与跟随镜头。", start: "开始录制", stop: "结束录制", wait: "请稍候…", shortcutHint: "随时开始或结束", detecting: "正在检测显示器", unlimited: "不限时长", watermark: "无水印 MP4", autoZoom: "自动缩放跟随", finalReady: "成片已准备好", openFolder: "打开文件夹", playFinal: "播放最终视频", playSource: "播放原始视频", settingsLead: "管理保存位置、录制屏幕和全局快捷键。", saveLocation: "保存位置", choose: "选择", display: "录制显示器", displayHelp: "只捕获选择的这一块屏幕", shortcut: "全局快捷键", shortcutHelp: "同一个按键开始与结束", change: "更改", privacy: "本机私密处理", privacyHelp: "画面与鼠标数据不会上传", languageLead: "选择 FocusFlow 的界面语言。", followSystem: "跟随系统", current: "当前", updateLead: "查看当前安装版本。联网更新将在后续版本提供。", currentVersion: "当前版本", latestLocal: "当前已安装版本", updateSoon: "自动检查与联网更新即将推出", processingLocal: "正在本机完成处理，请保持 FocusFlow 运行", failed: "这次成片没有顺利生成", savedFiles: "查看已保存文件" },
  en: { record: "Record", settings: "Recording settings", language: "Language", update: "Updates", ready: "Ready", recording: "Recording", working: "Processing", eyebrow: "Automatic camera recorder", idleTitle: "Press record,", idleAccent: "FocusFlow handles the camera.", liveTitle: "Recording now,", liveAccent: "focus on your demo.", subtitle: "Record your actions and automatically create smooth zoom and camera motion.", start: "Start recording", stop: "Stop recording", wait: "Please wait…", shortcutHint: "Start or stop anytime", detecting: "Detecting displays", unlimited: "Unlimited duration", watermark: "Watermark-free MP4", autoZoom: "Automatic zoom", finalReady: "Your video is ready", openFolder: "Open folder", playFinal: "Play final video", playSource: "Play source video", settingsLead: "Manage the save folder, recorded display, and global shortcut.", saveLocation: "Save location", choose: "Choose", display: "Recorded display", displayHelp: "Capture only the selected display", shortcut: "Global shortcut", shortcutHelp: "The same key starts and stops", change: "Change", privacy: "Private local processing", privacyHelp: "Screen and mouse data never leave this device", languageLead: "Choose the FocusFlow interface language.", followSystem: "Follow system", current: "Current", updateLead: "View the installed version. Online updates will arrive later.", currentVersion: "Current version", latestLocal: "Installed on this computer", updateSoon: "Automatic update checks are coming soon", processingLocal: "Processing locally. Keep FocusFlow open.", failed: "The final video could not be created", savedFiles: "View saved files" },
  ja: { record: "録画", settings: "録画設定", language: "言語", update: "更新", ready: "準備完了", recording: "録画中", working: "処理中", eyebrow: "自動カメラ録画", idleTitle: "録画を押せば、", idleAccent: "カメラは FocusFlow に。", liveTitle: "録画中です。", liveAccent: "デモに集中してください。", subtitle: "操作を録画し、滑らかなズームと追従を自動生成します。", start: "録画開始", stop: "録画終了", wait: "お待ちください…", shortcutHint: "いつでも開始・終了", detecting: "ディスプレイを検出中", unlimited: "時間制限なし", watermark: "透かしなし MP4", autoZoom: "自動ズーム", finalReady: "動画の準備ができました", openFolder: "フォルダーを開く", playFinal: "完成動画を再生", playSource: "元動画を再生", settingsLead: "保存先、録画画面、グローバルキーを管理します。", saveLocation: "保存先", choose: "選択", display: "録画ディスプレイ", displayHelp: "選択した画面だけを録画", shortcut: "グローバルキー", shortcutHelp: "同じキーで開始と終了", change: "変更", privacy: "ローカル処理", privacyHelp: "画面とマウス情報はアップロードされません", languageLead: "FocusFlow の表示言語を選択します。", followSystem: "システムに従う", current: "選択中", updateLead: "現在のバージョンを表示します。オンライン更新は今後対応予定です。", currentVersion: "現在のバージョン", latestLocal: "このPCにインストール済み", updateSoon: "自動更新は今後対応予定", processingLocal: "ローカルで処理中です。FocusFlowを閉じないでください。", failed: "完成動画を生成できませんでした", savedFiles: "保存ファイルを見る" },
  fr: { record: "Enregistrer", settings: "Réglages d’enregistrement", language: "Langue", update: "Mises à jour", ready: "Prêt", recording: "Enregistrement", working: "Traitement", eyebrow: "Enregistreur à caméra automatique", idleTitle: "Lancez l’enregistrement,", idleAccent: "FocusFlow gère la caméra.", liveTitle: "Enregistrement en cours,", liveAccent: "concentrez-vous sur la démo.", subtitle: "Enregistrez vos actions et créez automatiquement des zooms fluides.", start: "Démarrer", stop: "Arrêter", wait: "Veuillez patienter…", shortcutHint: "Démarrer ou arrêter à tout moment", detecting: "Détection des écrans", unlimited: "Durée illimitée", watermark: "MP4 sans filigrane", autoZoom: "Zoom automatique", finalReady: "Votre vidéo est prête", openFolder: "Ouvrir le dossier", playFinal: "Lire la vidéo finale", playSource: "Lire la vidéo source", settingsLead: "Gérez le dossier, l’écran enregistré et le raccourci global.", saveLocation: "Emplacement", choose: "Choisir", display: "Écran enregistré", displayHelp: "Capturer uniquement l’écran sélectionné", shortcut: "Raccourci global", shortcutHelp: "La même touche démarre et arrête", change: "Modifier", privacy: "Traitement local privé", privacyHelp: "Les données ne quittent jamais cet appareil", languageLead: "Choisissez la langue de FocusFlow.", followSystem: "Suivre le système", current: "Actuel", updateLead: "Consultez la version installée. Les mises à jour en ligne arriveront plus tard.", currentVersion: "Version actuelle", latestLocal: "Installée sur cet ordinateur", updateSoon: "La mise à jour automatique arrive bientôt", processingLocal: "Traitement local en cours. Gardez FocusFlow ouvert.", failed: "La vidéo finale n’a pas pu être créée", savedFiles: "Voir les fichiers" },
  de: { record: "Aufnehmen", settings: "Aufnahmeeinstellungen", language: "Sprache", update: "Updates", ready: "Bereit", recording: "Aufnahme läuft", working: "Verarbeitung", eyebrow: "Automatische Kameraaufnahme", idleTitle: "Aufnahme starten,", idleAccent: "FocusFlow führt die Kamera.", liveTitle: "Aufnahme läuft,", liveAccent: "konzentriere dich auf die Demo.", subtitle: "Nimm deine Aktionen auf und erzeuge automatisch sanfte Zooms und Kamerafahrten.", start: "Aufnahme starten", stop: "Aufnahme beenden", wait: "Bitte warten…", shortcutHint: "Jederzeit starten oder beenden", detecting: "Bildschirme werden erkannt", unlimited: "Unbegrenzte Dauer", watermark: "MP4 ohne Wasserzeichen", autoZoom: "Automatischer Zoom", finalReady: "Dein Video ist fertig", openFolder: "Ordner öffnen", playFinal: "Finales Video abspielen", playSource: "Originalvideo abspielen", settingsLead: "Verwalte Speicherort, Aufnahmebildschirm und globales Tastenkürzel.", saveLocation: "Speicherort", choose: "Auswählen", display: "Aufnahmebildschirm", displayHelp: "Nur den ausgewählten Bildschirm aufnehmen", shortcut: "Globales Tastenkürzel", shortcutHelp: "Dieselbe Taste startet und beendet", change: "Ändern", privacy: "Private lokale Verarbeitung", privacyHelp: "Bild- und Mausdaten verlassen dieses Gerät nicht", languageLead: "Wähle die Sprache der FocusFlow-Oberfläche.", followSystem: "Systemsprache", current: "Aktuell", updateLead: "Zeigt die installierte Version. Online-Updates folgen später.", currentVersion: "Aktuelle Version", latestLocal: "Auf diesem Computer installiert", updateSoon: "Automatische Updates folgen in Kürze", processingLocal: "Lokale Verarbeitung. FocusFlow bitte geöffnet lassen.", failed: "Das finale Video konnte nicht erstellt werden", savedFiles: "Gespeicherte Dateien anzeigen" },
  es: { record: "Grabar", settings: "Ajustes de grabación", language: "Idioma", update: "Actualizaciones", ready: "Listo", recording: "Grabando", working: "Procesando", eyebrow: "Grabador con cámara automática", idleTitle: "Pulsa grabar,", idleAccent: "FocusFlow dirige la cámara.", liveTitle: "Grabando ahora,", liveAccent: "concéntrate en tu demostración.", subtitle: "Graba tus acciones y crea automáticamente zooms y movimientos suaves.", start: "Iniciar grabación", stop: "Detener grabación", wait: "Espera…", shortcutHint: "Inicia o detén en cualquier momento", detecting: "Detectando pantallas", unlimited: "Duración ilimitada", watermark: "MP4 sin marca de agua", autoZoom: "Zoom automático", finalReady: "Tu vídeo está listo", openFolder: "Abrir carpeta", playFinal: "Reproducir vídeo final", playSource: "Reproducir vídeo original", settingsLead: "Gestiona la carpeta, la pantalla y el atajo global.", saveLocation: "Ubicación", choose: "Elegir", display: "Pantalla de grabación", displayHelp: "Captura solo la pantalla seleccionada", shortcut: "Atajo global", shortcutHelp: "La misma tecla inicia y detiene", change: "Cambiar", privacy: "Procesamiento local privado", privacyHelp: "Los datos nunca salen de este dispositivo", languageLead: "Elige el idioma de la interfaz de FocusFlow.", followSystem: "Seguir el sistema", current: "Actual", updateLead: "Consulta la versión instalada. Las actualizaciones en línea llegarán después.", currentVersion: "Versión actual", latestLocal: "Instalada en este equipo", updateSoon: "Las actualizaciones automáticas llegarán pronto", processingLocal: "Procesando localmente. Mantén FocusFlow abierto.", failed: "No se pudo crear el vídeo final", savedFiles: "Ver archivos guardados" },
  ar: { record: "تسجيل", settings: "إعدادات التسجيل", language: "اللغة", update: "التحديثات", ready: "جاهز", recording: "جارٍ التسجيل", working: "جارٍ المعالجة", eyebrow: "مسجل بكاميرا تلقائية", idleTitle: "اضغط على التسجيل،", idleAccent: "ودع FocusFlow يدير الكاميرا.", liveTitle: "التسجيل جارٍ،", liveAccent: "ركز على العرض فقط.", subtitle: "سجل خطواتك وأنشئ تلقائيًا تكبيرًا وحركة كاميرا سلسة.", start: "بدء التسجيل", stop: "إيقاف التسجيل", wait: "يرجى الانتظار…", shortcutHint: "ابدأ أو أوقف في أي وقت", detecting: "جارٍ اكتشاف الشاشات", unlimited: "مدة غير محدودة", watermark: "MP4 بلا علامة مائية", autoZoom: "تكبير تلقائي", finalReady: "الفيديو جاهز", openFolder: "فتح المجلد", playFinal: "تشغيل الفيديو النهائي", playSource: "تشغيل الفيديو الأصلي", settingsLead: "إدارة مجلد الحفظ والشاشة المسجلة واختصار لوحة المفاتيح.", saveLocation: "موقع الحفظ", choose: "اختيار", display: "شاشة التسجيل", displayHelp: "التقاط الشاشة المحددة فقط", shortcut: "اختصار عام", shortcutHelp: "المفتاح نفسه للبدء والإيقاف", change: "تغيير", privacy: "معالجة محلية خاصة", privacyHelp: "بيانات الشاشة والفأرة لا تغادر جهازك", languageLead: "اختر لغة واجهة FocusFlow.", followSystem: "اتباع لغة النظام", current: "الحالي", updateLead: "عرض الإصدار المثبت. ستتوفر التحديثات عبر الإنترنت لاحقًا.", currentVersion: "الإصدار الحالي", latestLocal: "مثبت على هذا الجهاز", updateSoon: "التحديثات التلقائية قادمة قريبًا", processingLocal: "تتم المعالجة محليًا. أبقِ FocusFlow مفتوحًا.", failed: "تعذر إنشاء الفيديو النهائي", savedFiles: "عرض الملفات المحفوظة" },
} as const;

function systemLanguage(): ResolvedLanguage {
  const locale = navigator.language.toLowerCase();
  if (locale.startsWith("zh")) return "zh";
  if (locale.startsWith("ja")) return "ja";
  if (locale.startsWith("fr")) return "fr";
  if (locale.startsWith("de")) return "de";
  if (locale.startsWith("es")) return "es";
  if (locale.startsWith("ar")) return "ar";
  return "en";
}

function readSavedLanguage(): AppLanguage {
  const saved = localStorage.getItem("focusflow.language");
  return saved === "zh" || saved === "en" || saved === "ja" || saved === "fr" || saved === "de" || saved === "es" || saved === "ar" ? saved : "system";
}

function Glyph({ name }: { name: AppPage }) {
  const paths = {
    record: <><circle cx="12" cy="12" r="7" /><circle cx="12" cy="12" r="2.5" fill="currentColor" /></>,
    settings: <><path d="M4 7h10M18 7h2M4 17h2M10 17h10M14 4v6M7 14v6" /></>,
    language: <><circle cx="12" cy="12" r="9" /><path d="M3 12h18M12 3c3 3 3 15 0 18M12 3c-3 3-3 15 0 18" /></>,
    update: <><path d="M20 11a8 8 0 1 0-2.3 5.7" /><path d="M20 5v6h-6" /></>,
  };
  return <svg viewBox="0 0 24 24" aria-hidden="true">{paths[name]}</svg>;
}

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
  const [activePage, setActivePage] = useState<AppPage>("record");
  const [language, setLanguage] = useState<AppLanguage>(readSavedLanguage);
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
  const shortcutQueueRef = useRef<Promise<void>>(Promise.resolve());
  const resolvedLanguage = language === "system" ? systemLanguage() : language;
  const t = translations[resolvedLanguage];

  useEffect(() => { statusRef.current = status; }, [status]);
  useEffect(() => { busyRef.current = busy; }, [busy]);
  useEffect(() => { shortcutRef.current = shortcut; }, [shortcut]);
  useEffect(() => { shortcutDialogRef.current = shortcutDialogOpen; }, [shortcutDialogOpen]);
  useEffect(() => {
    document.documentElement.lang = resolvedLanguage;
    document.documentElement.dir = resolvedLanguage === "ar" ? "rtl" : "ltr";
    return () => { document.documentElement.dir = "ltr"; };
  }, [resolvedLanguage]);

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
    let active = true;
    shortcutQueueRef.current = shortcutQueueRef.current
      .catch(() => undefined)
      .then(async () => {
        await unregister(shortcut).catch(() => undefined);
        if (!active) return;
        await register(shortcut, (event) => {
          if (event.state === "Pressed" && !shortcutDialogRef.current) void toggleRef.current();
        });
      })
      .catch((error) => {
        if (active) setMessage(`快捷键不可用：${String(error)}`);
      });
    return () => {
      active = false;
      shortcutQueueRef.current = shortcutQueueRef.current
        .catch(() => undefined)
        .then(() => unregister(shortcut).catch(() => undefined));
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
    if (busy) return t.wait;
    return status.isRecording ? t.stop : t.start;
  }, [busy, status.isRecording, t]);

  function changeLanguage(next: AppLanguage) {
    setLanguage(next);
    localStorage.setItem("focusflow.language", next);
  }

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
    setShortcut(capturedShortcut);
    shortcutRef.current = capturedShortcut;
    localStorage.setItem("focusflow.shortcut", capturedShortcut);
    setMessage(`开始与结束快捷键已设为 ${capturedShortcut}`);
    setShortcutDialogOpen(false);
    setCapturedShortcut(null);
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
      <section className="app-frame product-shell">
        <aside className="side-rail">
          <button className={`rail-brand ${activePage === "record" ? "active" : ""}`} type="button" aria-label={t.record} onClick={() => setActivePage("record")}><span className="brand-mark"><i /></span></button>
          <nav className="rail-navigation" aria-label="FocusFlow">
            {(["settings", "language", "update"] as AppPage[]).map((page) => (
              <button key={page} type="button" className={`rail-button ${activePage === page ? "active" : ""}`} title={t[page]} aria-label={t[page]} aria-current={activePage === page ? "page" : undefined} onClick={() => setActivePage(page)}><Glyph name={page} /><span>{t[page]}</span></button>
            ))}
          </nav>
          <span className="rail-version">v{APP_VERSION}</span>
        </aside>

        <section className="app-main">
          <header className="main-header">
            <div className="brand-copy"><strong>FocusFlow</strong><small>AUTOMATIC CAMERA RECORDER</small></div>
            <span className="alpha-badge">ALPHA</span>
            <div className={`status-pill ${status.isRecording ? "live" : busy ? "working" : "ready"}`}><span />{status.isRecording ? t.recording : busy ? t.working : t.ready}</div>
          </header>

          <div className="page-stage">
            {activePage === "record" && (
              <section className="hero record-page">
                <div className="hero-ambient one" /><div className="hero-ambient two" />
                <p className="eyebrow"><i /> {t.eyebrow.toUpperCase()}</p>
                <h1>{status.isRecording ? <>{t.liveTitle}<em>{t.liveAccent}</em></> : <>{t.idleTitle}<em>{t.idleAccent}</em></>}</h1>
                <p className="hero-subtitle">{t.subtitle}</p>
                <p className="message"><span />{message}</p>
                <div className={`record-console ${status.isRecording ? "is-recording" : ""}`}>
                  <div className="capture-main">
                    <div className="timer-block"><span className="timer-label">{status.isRecording ? "RECORDING" : "DURATION"}</span><strong className="timer"><i />{formatElapsed(elapsed)}</strong></div>
                    <button type="button" className="primary-button record-action" disabled={busy || (!status.ffmpegAvailable && !status.isRecording)} onClick={() => void toggleRecording()}><span className="record-glyph">{status.isRecording ? "■" : ""}</span>{actionLabel}</button>
                  </div>
                  <div className="capture-foot"><span><kbd>{shortcut}</kbd> {t.shortcutHint}</span><span>{selectedDisplay ? selectedDisplay.label : t.detecting}</span></div>
                </div>
                <div className="outcome-slot">
                  {busy && processing && <section className="processing-card" aria-live="polite"><div className="processing-copy"><span className="spinner" /><div><strong>{processing.message}</strong><span>{t.processingLocal}</span></div><b>{processing.progress}%</b></div><div className="progress-track"><i style={{ width: `${processing.progress}%` }} /></div></section>}
                  {!busy && failure && <section className="failure-card" role="alert"><div><strong>{t.failed}</strong><span>{failure}</span></div><button type="button" className="secondary-button" onClick={() => void revealOutput()}>{t.savedFiles}</button></section>}
                  {!busy && !failure && status.finalVideoPath && <section className="success-card"><div className="success-icon">✓</div><div className="success-copy"><strong>{t.finalReady}</strong><span title={status.finalVideoPath}>{fileName(status.finalVideoPath)}</span></div><div className="result-actions"><button type="button" className="secondary-button" onClick={() => void revealOutput()}>{t.openFolder}</button><button type="button" className="primary-button compact" onClick={() => void openFinalVideo()}>▶ {t.playFinal}</button><button type="button" className="secondary-button" onClick={() => void openSourceVideo()}>{t.playSource}</button></div></section>}
                  {!busy && !failure && !status.finalVideoPath && <div className="promise-row"><span>∞ {t.unlimited}</span><span>◇ {t.watermark}</span><span>⌁ {t.autoZoom}</span></div>}
                </div>
              </section>
            )}

            {activePage === "settings" && (
              <section className="content-page">
                <div className="page-heading"><span className="section-kicker">01 / RECORDING</span><h1>{t.settings}</h1><p>{t.settingsLead}</p></div>
                <div className="settings-panel">
                  <div className="setting-row"><span className="setting-index">01</span><div className="setting-copy"><strong>{t.saveLocation}</strong><span title={status.outputDirectory}>{status.outputDirectory || "—"}</span></div><button type="button" className="secondary-button" disabled={status.isRecording || busy} onClick={() => void chooseOutputDirectory()}>{t.choose}</button></div>
                  <div className="setting-row"><span className="setting-index">02</span><div className="setting-copy"><strong>{t.display}</strong><span>{t.displayHelp}</span></div><select className="display-select" value={selectedDisplayId} disabled={status.isRecording || busy} onChange={(event) => changeDisplay(event.target.value)}>{displays.map((display) => <option key={display.id} value={display.id}>{display.label}</option>)}</select></div>
                  <div className="setting-row"><span className="setting-index">03</span><div className="setting-copy"><strong>{t.shortcut}</strong><span>{t.shortcutHelp}</span></div><div className="shortcut-control"><kbd>{shortcut}</kbd><button type="button" className="secondary-button" disabled={status.isRecording || busy} onClick={() => { setCapturedShortcut(null); setShortcutDialogOpen(true); }}>{t.change}</button></div></div>
                  <div className="privacy-note"><span>⌾</span><div><strong>{t.privacy}</strong><small>{t.privacyHelp}</small></div></div>
                </div>
              </section>
            )}

            {activePage === "language" && (
              <section className="content-page">
                <div className="page-heading"><span className="section-kicker">02 / LANGUAGE</span><h1>{t.language}</h1><p>{t.languageLead}</p></div>
                <div className="language-grid">
                  {([{ id: "system", name: t.followSystem, meta: navigator.language }, { id: "zh", name: "中文", meta: "简体中文" }, { id: "en", name: "English", meta: "English" }, { id: "ja", name: "日本語", meta: "日本語" }, { id: "fr", name: "Français", meta: "Français" }, { id: "de", name: "Deutsch", meta: "Deutsch" }, { id: "es", name: "Español", meta: "Español" }, { id: "ar", name: "العربية", meta: "العربية · RTL" }] as { id: AppLanguage; name: string; meta: string }[]).map((option) => <button key={option.id} type="button" className={`language-option ${language === option.id ? "active" : ""}`} onClick={() => changeLanguage(option.id)}><span><strong>{option.name}</strong><small>{option.meta}</small></span><i>{language === option.id ? "✓" : ""}</i></button>)}
                </div>
              </section>
            )}

            {activePage === "update" && (
              <section className="content-page">
                <div className="page-heading"><span className="section-kicker">03 / UPDATE</span><h1>{t.update}</h1><p>{t.updateLead}</p></div>
                <div className="version-card"><div className="version-icon"><Glyph name="update" /></div><div><span>{t.currentVersion}</span><strong>FocusFlow v{APP_VERSION}</strong><small>{t.latestLocal}</small></div><b>{t.current}</b></div>
                <p className="coming-soon"><span />{t.updateSoon}</p>
              </section>
            )}
          </div>

          <footer><span>FocusFlow Alpha · Built for Windows</span><div><span>LOCAL PROCESSING</span><i /> <span>H.264 MP4</span><i /> <span>NO WATERMARK</span></div></footer>
        </section>
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
