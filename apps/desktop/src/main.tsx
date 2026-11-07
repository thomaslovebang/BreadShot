import { convertFileSrc, invoke } from "@tauri-apps/api/core";
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

type AudioDevice = { id: string; name: string; isDefault: boolean };
type AudioCapabilities = { systemAudioAvailable: boolean; microphones: AudioDevice[] };

type AppPage = "record" | "settings" | "effects" | "language" | "update";
type AppLanguage = "system" | "zh" | "en" | "ja" | "fr" | "de" | "es" | "ar";
type ResolvedLanguage = Exclude<AppLanguage, "system">;

const DEFAULT_SHORTCUT = "F8";
const APP_VERSION = "0.3.2";
const SINGLE_KEY_PATTERN = /^(?:[A-Z0-9]|F(?:[1-9]|1\d|2[0-4]))$/;
const initialStatus: RecorderStatus = { isRecording: false, ffmpegAvailable: false, outputPath: null, outputDirectory: "", finalVideoPath: null };

const cursorRoles = [
  ["normal", "正常选择"], ["help", "帮助选择"], ["app_starting", "后台运行"], ["wait", "忙"],
  ["crosshair", "精确选择"], ["text", "文本选择"], ["handwriting", "手写"], ["unavailable", "不可用"],
  ["vertical_resize", "垂直调整大小"], ["horizontal_resize", "水平调整大小"], ["diagonal_resize_1", "沿对角线调整 1"],
  ["diagonal_resize_2", "沿对角线调整 2"], ["move", "移动"], ["alternate", "候选"], ["link", "链接选择"],
  ["location", "位置选择"], ["person", "个人选择"],
] as const;
type CursorRole = typeof cursorRoles[number][0];
type CursorPreview = { path: string; hotspotX: number; hotspotY: number; width: number; height: number };
type PageBackgroundSettings = { fit: "cover" | "contain"; positionX: number; positionY: number; zoom: number };

const defaultPageBackgroundSettings: PageBackgroundSettings = { fit: "cover", positionX: 50, positionY: 50, zoom: 1 };

function readPageBackgroundSettings(): PageBackgroundSettings {
  try {
    const saved = JSON.parse(localStorage.getItem("focusflow.pageBackgroundSettings") || "null");
    if (saved && (saved.fit === "cover" || saved.fit === "contain")) {
      return {
        fit: saved.fit,
        positionX: Math.min(100, Math.max(0, Number(saved.positionX) || 50)),
        positionY: Math.min(100, Math.max(0, Number(saved.positionY) || 50)),
        zoom: Math.min(2.5, Math.max(1, Number(saved.zoom) || 1)),
      };
    }
  } catch { /* use defaults */ }
  return defaultPageBackgroundSettings;
}

const effectsTranslations: Record<ResolvedLanguage, { nav: string; title: string; lead: string; custom: string; customHelp: string; size: string; sizeHelp: string; choose: string; restore: string; resetSize: string; windowsDefault: string; preview: string }> = {
  zh: { nav: "特效设置", title: "特效设置", lead: "只改变导出视频中的鼠标指针，不修改 Windows 系统设置。", custom: "自定义鼠标指针", customHelp: "可单独替换任意角色的 CUR 或 ANI 文件", size: "鼠标大小", sizeHelp: "全局等比例应用到全部指针角色", choose: "选择文件", restore: "恢复默认", resetSize: "恢复默认大小", windowsDefault: "Windows 默认", preview: "预览角色" },
  en: { nav: "Effects", title: "Effects", lead: "Changes only the pointer in exported videos, never Windows settings.", custom: "Custom pointers", customHelp: "Replace any role with a CUR or ANI file", size: "Pointer size", sizeHelp: "Scales every pointer role proportionally", choose: "Choose file", restore: "Restore", resetSize: "Default size", windowsDefault: "Windows default", preview: "Preview role" },
  ja: { nav: "エフェクト", title: "エフェクト設定", lead: "書き出す動画だけを変更し、Windows設定は変更しません。", custom: "カスタムポインター", customHelp: "各役割に CUR / ANI を設定", size: "ポインターサイズ", sizeHelp: "全役割を同じ倍率で拡大縮小", choose: "ファイル選択", restore: "既定に戻す", resetSize: "既定のサイズ", windowsDefault: "Windows既定", preview: "プレビュー" },
  fr: { nav: "Effets", title: "Effets", lead: "Modifie uniquement le pointeur de la vidéo exportée.", custom: "Pointeurs personnalisés", customHelp: "CUR ou ANI pour chaque rôle", size: "Taille du pointeur", sizeHelp: "Échelle globale pour tous les rôles", choose: "Choisir", restore: "Restaurer", resetSize: "Taille par défaut", windowsDefault: "Windows par défaut", preview: "Aperçu" },
  de: { nav: "Effekte", title: "Effekte", lead: "Ändert nur den Zeiger im exportierten Video.", custom: "Eigene Zeiger", customHelp: "CUR oder ANI für jede Rolle", size: "Zeigergröße", sizeHelp: "Globale Skalierung aller Rollen", choose: "Auswählen", restore: "Zurücksetzen", resetSize: "Standardgröße", windowsDefault: "Windows-Standard", preview: "Vorschau" },
  es: { nav: "Efectos", title: "Efectos", lead: "Solo cambia el puntero del vídeo exportado.", custom: "Punteros personalizados", customHelp: "CUR o ANI para cada función", size: "Tamaño", sizeHelp: "Escala global para todos los punteros", choose: "Elegir", restore: "Restaurar", resetSize: "Tamaño predeterminado", windowsDefault: "Windows predeterminado", preview: "Vista previa" },
  ar: { nav: "التأثيرات", title: "إعدادات التأثيرات", lead: "يغير المؤشر في الفيديو المصدر فقط دون إعدادات Windows.", custom: "مؤشرات مخصصة", customHelp: "ملف CUR أو ANI لكل دور", size: "حجم المؤشر", sizeHelp: "تحجيم موحد لجميع المؤشرات", choose: "اختيار", restore: "استعادة", resetSize: "الحجم الافتراضي", windowsDefault: "Windows الافتراضي", preview: "معاينة" },
};

const pageBackgroundTranslations = {
  zh: { title: "自定义页面", help: "使用自己的图片装饰整个应用，不会出现在录制和成片中。", choose: "选择图片", adjust: "调整", restore: "恢复默认", modal: "调整应用背景", fill: "填满页面", full: "完整显示", fillHelp: "拖动图片选择显示区域，可使用滚轮或滑块缩放。", fullHelp: "完整显示图片，比例不一致时可能留边。", zoom: "缩放", cancel: "取消", save: "应用" },
  en: { title: "Custom appearance", help: "Use your image across the entire app. It never appears in recordings.", choose: "Choose image", adjust: "Adjust", restore: "Restore default", modal: "Adjust app background", fill: "Fill page", full: "Show full image", fillHelp: "Drag to choose the visible area, then zoom with the wheel or slider.", fullHelp: "Shows the full image and may leave space when aspect ratios differ.", zoom: "Zoom", cancel: "Cancel", save: "Apply" },
  ja: { title: "画面をカスタマイズ", help: "アプリ全体を画像で飾ります。録画や完成動画には入りません。", choose: "画像を選択", adjust: "調整", restore: "既定に戻す", modal: "アプリ背景を調整", fill: "画面を埋める", full: "画像全体を表示", fillHelp: "ドラッグで表示範囲を選び、ホイールまたはスライダーで拡大します。", fullHelp: "画像全体を表示し、比率が異なる場合は余白ができます。", zoom: "拡大", cancel: "キャンセル", save: "適用" },
  fr: { title: "Apparence personnalisée", help: "Utilisez votre image dans toute l’application, sans l’inclure dans les enregistrements.", choose: "Choisir une image", adjust: "Ajuster", restore: "Réinitialiser", modal: "Ajuster l’arrière-plan", fill: "Remplir la page", full: "Afficher l’image entière", fillHelp: "Faites glisser pour choisir la zone visible et utilisez le zoom.", fullHelp: "Affiche toute l’image avec d’éventuelles marges.", zoom: "Zoom", cancel: "Annuler", save: "Appliquer" },
  de: { title: "Eigene Oberfläche", help: "Verwende dein Bild in der gesamten App; es erscheint nicht in Aufnahmen.", choose: "Bild auswählen", adjust: "Anpassen", restore: "Standard", modal: "App-Hintergrund anpassen", fill: "Seite füllen", full: "Ganzes Bild", fillHelp: "Ziehe das Bild, um den Ausschnitt zu wählen, und passe den Zoom an.", fullHelp: "Zeigt das ganze Bild und kann Ränder lassen.", zoom: "Zoom", cancel: "Abbrechen", save: "Anwenden" },
  es: { title: "Apariencia personalizada", help: "Usa tu imagen en toda la aplicación sin incluirla en el vídeo.", choose: "Elegir imagen", adjust: "Ajustar", restore: "Restaurar", modal: "Ajustar fondo", fill: "Llenar página", full: "Mostrar completa", fillHelp: "Arrastra para elegir el área visible y ajusta el zoom.", fullHelp: "Muestra toda la imagen y puede dejar márgenes.", zoom: "Zoom", cancel: "Cancelar", save: "Aplicar" },
  ar: { title: "مظهر مخصص", help: "استخدم صورتك في التطبيق بالكامل دون ظهورها في التسجيل أو الفيديو النهائي.", choose: "اختيار صورة", adjust: "ضبط", restore: "استعادة الافتراضي", modal: "ضبط خلفية التطبيق", fill: "ملء الصفحة", full: "عرض الصورة كاملة", fillHelp: "اسحب لاختيار الجزء الظاهر واضبط التكبير.", fullHelp: "يعرض الصورة كاملة وقد يترك هوامش.", zoom: "التكبير", cancel: "إلغاء", save: "تطبيق" },
} as const;

function readCursorPaths(): Partial<Record<CursorRole, string>> {
  try { return JSON.parse(localStorage.getItem("focusflow.cursorPaths") || "{}"); } catch { return {}; }
}

function readCursorSize() {
  const value = Number(localStorage.getItem("focusflow.cursorSize") || 100);
  return Number.isFinite(value) ? Math.min(250, Math.max(50, value)) : 100;
}

const translations = {
  zh: { record: "录制", settings: "录制设置", language: "语言", update: "更新", ready: "可以开始", recording: "正在录制", working: "正在处理", eyebrow: "自动运镜录屏", idleTitle: "按下录制，", idleAccent: "镜头交给 BreadShot。", liveTitle: "正在录制，", liveAccent: "专注演示就好。", subtitle: "录制你的操作，自动生成平滑缩放与跟随镜头。", start: "开始录制", stop: "结束录制", wait: "请稍候…", shortcutHint: "随时开始或结束", detecting: "正在检测显示器", unlimited: "不限时长", watermark: "无水印 MP4", autoZoom: "自动缩放跟随", finalReady: "成片已准备好", openFolder: "打开文件夹", playFinal: "播放最终视频", playSource: "播放原始视频", settingsLead: "管理保存位置、录制屏幕和全局快捷键。", saveLocation: "保存位置", choose: "选择", display: "录制显示器", displayHelp: "只捕获选择的这一块屏幕", shortcut: "全局快捷键", shortcutHelp: "同一个按键开始与结束", change: "更改", privacy: "本机私密处理", privacyHelp: "画面与鼠标数据不会上传", languageLead: "选择 BreadShot 的界面语言。", followSystem: "跟随系统", current: "当前", updateLead: "查看当前安装版本。联网更新将在后续版本提供。", currentVersion: "当前版本", latestLocal: "当前已安装版本", updateSoon: "自动检查与联网更新即将推出", processingLocal: "正在本机完成处理，请保持 BreadShot 运行", failed: "这次成片没有顺利生成", savedFiles: "查看已保存文件" },
  en: { record: "Record", settings: "Recording settings", language: "Language", update: "Updates", ready: "Ready", recording: "Recording", working: "Processing", eyebrow: "Automatic camera recorder", idleTitle: "Press record,", idleAccent: "BreadShot handles the camera.", liveTitle: "Recording now,", liveAccent: "focus on your demo.", subtitle: "Record your actions and automatically create smooth zoom and camera motion.", start: "Start recording", stop: "Stop recording", wait: "Please wait…", shortcutHint: "Start or stop anytime", detecting: "Detecting displays", unlimited: "Unlimited duration", watermark: "Watermark-free MP4", autoZoom: "Automatic zoom", finalReady: "Your video is ready", openFolder: "Open folder", playFinal: "Play final video", playSource: "Play source video", settingsLead: "Manage the save folder, recorded display, and global shortcut.", saveLocation: "Save location", choose: "Choose", display: "Recorded display", displayHelp: "Capture only the selected display", shortcut: "Global shortcut", shortcutHelp: "The same key starts and stops", change: "Change", privacy: "Private local processing", privacyHelp: "Screen and mouse data never leave this device", languageLead: "Choose the BreadShot interface language.", followSystem: "Follow system", current: "Current", updateLead: "View the installed version. Online updates will arrive later.", currentVersion: "Current version", latestLocal: "Installed on this computer", updateSoon: "Automatic update checks are coming soon", processingLocal: "Processing locally. Keep BreadShot open.", failed: "The final video could not be created", savedFiles: "View saved files" },
  ja: { record: "録画", settings: "録画設定", language: "言語", update: "更新", ready: "準備完了", recording: "録画中", working: "処理中", eyebrow: "自動カメラ録画", idleTitle: "録画を押せば、", idleAccent: "カメラは BreadShot に。", liveTitle: "録画中です。", liveAccent: "デモに集中してください。", subtitle: "操作を録画し、滑らかなズームと追従を自動生成します。", start: "録画開始", stop: "録画終了", wait: "お待ちください…", shortcutHint: "いつでも開始・終了", detecting: "ディスプレイを検出中", unlimited: "時間制限なし", watermark: "透かしなし MP4", autoZoom: "自動ズーム", finalReady: "動画の準備ができました", openFolder: "フォルダーを開く", playFinal: "完成動画を再生", playSource: "元動画を再生", settingsLead: "保存先、録画画面、グローバルキーを管理します。", saveLocation: "保存先", choose: "選択", display: "録画ディスプレイ", displayHelp: "選択した画面だけを録画", shortcut: "グローバルキー", shortcutHelp: "同じキーで開始と終了", change: "変更", privacy: "ローカル処理", privacyHelp: "画面とマウス情報はアップロードされません", languageLead: "BreadShot の表示言語を選択します。", followSystem: "システムに従う", current: "選択中", updateLead: "現在のバージョンを表示します。オンライン更新は今後対応予定です。", currentVersion: "現在のバージョン", latestLocal: "このPCにインストール済み", updateSoon: "自動更新は今後対応予定", processingLocal: "ローカルで処理中です。BreadShotを閉じないでください。", failed: "完成動画を生成できませんでした", savedFiles: "保存ファイルを見る" },
  fr: { record: "Enregistrer", settings: "Réglages d’enregistrement", language: "Langue", update: "Mises à jour", ready: "Prêt", recording: "Enregistrement", working: "Traitement", eyebrow: "Enregistreur à caméra automatique", idleTitle: "Lancez l’enregistrement,", idleAccent: "BreadShot gère la caméra.", liveTitle: "Enregistrement en cours,", liveAccent: "concentrez-vous sur la démo.", subtitle: "Enregistrez vos actions et créez automatiquement des zooms fluides.", start: "Démarrer", stop: "Arrêter", wait: "Veuillez patienter…", shortcutHint: "Démarrer ou arrêter à tout moment", detecting: "Détection des écrans", unlimited: "Durée illimitée", watermark: "MP4 sans filigrane", autoZoom: "Zoom automatique", finalReady: "Votre vidéo est prête", openFolder: "Ouvrir le dossier", playFinal: "Lire la vidéo finale", playSource: "Lire la vidéo source", settingsLead: "Gérez le dossier, l’écran enregistré et le raccourci global.", saveLocation: "Emplacement", choose: "Choisir", display: "Écran enregistré", displayHelp: "Capturer uniquement l’écran sélectionné", shortcut: "Raccourci global", shortcutHelp: "La même touche démarre et arrête", change: "Modifier", privacy: "Traitement local privé", privacyHelp: "Les données ne quittent jamais cet appareil", languageLead: "Choisissez la langue de BreadShot.", followSystem: "Suivre le système", current: "Actuel", updateLead: "Consultez la version installée. Les mises à jour en ligne arriveront plus tard.", currentVersion: "Version actuelle", latestLocal: "Installée sur cet ordinateur", updateSoon: "La mise à jour automatique arrive bientôt", processingLocal: "Traitement local en cours. Gardez BreadShot ouvert.", failed: "La vidéo finale n’a pas pu être créée", savedFiles: "Voir les fichiers" },
  de: { record: "Aufnehmen", settings: "Aufnahmeeinstellungen", language: "Sprache", update: "Updates", ready: "Bereit", recording: "Aufnahme läuft", working: "Verarbeitung", eyebrow: "Automatische Kameraaufnahme", idleTitle: "Aufnahme starten,", idleAccent: "BreadShot führt die Kamera.", liveTitle: "Aufnahme läuft,", liveAccent: "konzentriere dich auf die Demo.", subtitle: "Nimm deine Aktionen auf und erzeuge automatisch sanfte Zooms und Kamerafahrten.", start: "Aufnahme starten", stop: "Aufnahme beenden", wait: "Bitte warten…", shortcutHint: "Jederzeit starten oder beenden", detecting: "Bildschirme werden erkannt", unlimited: "Unbegrenzte Dauer", watermark: "MP4 ohne Wasserzeichen", autoZoom: "Automatischer Zoom", finalReady: "Dein Video ist fertig", openFolder: "Ordner öffnen", playFinal: "Finales Video abspielen", playSource: "Originalvideo abspielen", settingsLead: "Verwalte Speicherort, Aufnahmebildschirm und globales Tastenkürzel.", saveLocation: "Speicherort", choose: "Auswählen", display: "Aufnahmebildschirm", displayHelp: "Nur den ausgewählten Bildschirm aufnehmen", shortcut: "Globales Tastenkürzel", shortcutHelp: "Dieselbe Taste startet und beendet", change: "Ändern", privacy: "Private lokale Verarbeitung", privacyHelp: "Bild- und Mausdaten verlassen dieses Gerät nicht", languageLead: "Wähle die Sprache der BreadShot-Oberfläche.", followSystem: "Systemsprache", current: "Aktuell", updateLead: "Zeigt die installierte Version. Online-Updates folgen später.", currentVersion: "Aktuelle Version", latestLocal: "Auf diesem Computer installiert", updateSoon: "Automatische Updates folgen in Kürze", processingLocal: "Lokale Verarbeitung. BreadShot bitte geöffnet lassen.", failed: "Das finale Video konnte nicht erstellt werden", savedFiles: "Gespeicherte Dateien anzeigen" },
  es: { record: "Grabar", settings: "Ajustes de grabación", language: "Idioma", update: "Actualizaciones", ready: "Listo", recording: "Grabando", working: "Procesando", eyebrow: "Grabador con cámara automática", idleTitle: "Pulsa grabar,", idleAccent: "BreadShot dirige la cámara.", liveTitle: "Grabando ahora,", liveAccent: "concéntrate en tu demostración.", subtitle: "Graba tus acciones y crea automáticamente zooms y movimientos suaves.", start: "Iniciar grabación", stop: "Detener grabación", wait: "Espera…", shortcutHint: "Inicia o detén en cualquier momento", detecting: "Detectando pantallas", unlimited: "Duración ilimitada", watermark: "MP4 sin marca de agua", autoZoom: "Zoom automático", finalReady: "Tu vídeo está listo", openFolder: "Abrir carpeta", playFinal: "Reproducir vídeo final", playSource: "Reproducir vídeo original", settingsLead: "Gestiona la carpeta, la pantalla y el atajo global.", saveLocation: "Ubicación", choose: "Elegir", display: "Pantalla de grabación", displayHelp: "Captura solo la pantalla seleccionada", shortcut: "Atajo global", shortcutHelp: "La misma tecla inicia y detiene", change: "Cambiar", privacy: "Procesamiento local privado", privacyHelp: "Los datos nunca salen de este dispositivo", languageLead: "Elige el idioma de la interfaz de BreadShot.", followSystem: "Seguir el sistema", current: "Actual", updateLead: "Consulta la versión instalada. Las actualizaciones en línea llegarán después.", currentVersion: "Versión actual", latestLocal: "Instalada en este equipo", updateSoon: "Las actualizaciones automáticas llegarán pronto", processingLocal: "Procesando localmente. Mantén BreadShot abierto.", failed: "No se pudo crear el vídeo final", savedFiles: "Ver archivos guardados" },
  ar: { record: "تسجيل", settings: "إعدادات التسجيل", language: "اللغة", update: "التحديثات", ready: "جاهز", recording: "جارٍ التسجيل", working: "جارٍ المعالجة", eyebrow: "مسجل بكاميرا تلقائية", idleTitle: "اضغط على التسجيل،", idleAccent: "ودع BreadShot يدير الكاميرا.", liveTitle: "التسجيل جارٍ،", liveAccent: "ركز على العرض فقط.", subtitle: "سجل خطواتك وأنشئ تلقائيًا تكبيرًا وحركة كاميرا سلسة.", start: "بدء التسجيل", stop: "إيقاف التسجيل", wait: "يرجى الانتظار…", shortcutHint: "ابدأ أو أوقف في أي وقت", detecting: "جارٍ اكتشاف الشاشات", unlimited: "مدة غير محدودة", watermark: "MP4 بلا علامة مائية", autoZoom: "تكبير تلقائي", finalReady: "الفيديو جاهز", openFolder: "فتح المجلد", playFinal: "تشغيل الفيديو النهائي", playSource: "تشغيل الفيديو الأصلي", settingsLead: "إدارة مجلد الحفظ والشاشة المسجلة واختصار لوحة المفاتيح.", saveLocation: "موقع الحفظ", choose: "اختيار", display: "شاشة التسجيل", displayHelp: "التقاط الشاشة المحددة فقط", shortcut: "اختصار عام", shortcutHelp: "المفتاح نفسه للبدء والإيقاف", change: "تغيير", privacy: "معالجة محلية خاصة", privacyHelp: "بيانات الشاشة والفأرة لا تغادر جهازك", languageLead: "اختر لغة واجهة BreadShot.", followSystem: "اتباع لغة النظام", current: "الحالي", updateLead: "عرض الإصدار المثبت. ستتوفر التحديثات عبر الإنترنت لاحقًا.", currentVersion: "الإصدار الحالي", latestLocal: "مثبت على هذا الجهاز", updateSoon: "التحديثات التلقائية قادمة قريبًا", processingLocal: "تتم المعالجة محليًا. أبقِ BreadShot مفتوحًا.", failed: "تعذر إنشاء الفيديو النهائي", savedFiles: "عرض الملفات المحفوظة" },
} as const;

const audioTranslations = {
  zh: { audio: "声音", system: "系统声音", systemHelp: "录制电脑正在播放的声音", microphone: "麦克风", microphoneHelp: "录制旁白并与系统声音混合", unavailable: "不可用", noMicrophone: "未检测到麦克风", on: "开启", off: "关闭", defaultDevice: "默认设备" },
  en: { audio: "Audio", system: "System audio", systemHelp: "Capture sound playing on this computer", microphone: "Microphone", microphoneHelp: "Record narration and mix it with system audio", unavailable: "Unavailable", noMicrophone: "No microphone detected", on: "On", off: "Off", defaultDevice: "Default device" },
  ja: { audio: "音声", system: "システム音声", systemHelp: "PCで再生される音声を録音", microphone: "マイク", microphoneHelp: "ナレーションを録音してシステム音声とミックス", unavailable: "利用不可", noMicrophone: "マイクが見つかりません", on: "オン", off: "オフ", defaultDevice: "既定のデバイス" },
  fr: { audio: "Audio", system: "Audio système", systemHelp: "Capturer le son lu sur cet ordinateur", microphone: "Microphone", microphoneHelp: "Enregistrer la narration et la mixer avec le son système", unavailable: "Indisponible", noMicrophone: "Aucun microphone détecté", on: "Activé", off: "Désactivé", defaultDevice: "Périphérique par défaut" },
  de: { audio: "Audio", system: "Systemaudio", systemHelp: "Ton aufnehmen, der auf diesem Computer wiedergegeben wird", microphone: "Mikrofon", microphoneHelp: "Kommentar aufnehmen und mit Systemaudio mischen", unavailable: "Nicht verfügbar", noMicrophone: "Kein Mikrofon erkannt", on: "Ein", off: "Aus", defaultDevice: "Standardgerät" },
  es: { audio: "Audio", system: "Audio del sistema", systemHelp: "Captura el sonido reproducido en este equipo", microphone: "Micrófono", microphoneHelp: "Graba narración y mézclala con el audio del sistema", unavailable: "No disponible", noMicrophone: "No se detectó ningún micrófono", on: "Activado", off: "Desactivado", defaultDevice: "Dispositivo predeterminado" },
  ar: { audio: "الصوت", system: "صوت النظام", systemHelp: "تسجيل الصوت الذي يشغله الكمبيوتر", microphone: "الميكروفون", microphoneHelp: "تسجيل التعليق ودمجه مع صوت النظام", unavailable: "غير متاح", noMicrophone: "لم يتم اكتشاف ميكروفون", on: "تشغيل", off: "إيقاف", defaultDevice: "الجهاز الافتراضي" },
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
    effects: <><path d="m12 3 1.5 4.2L18 8.5l-3.5 2.7.2 4.6-3.7-2.6-4 1.6 1.3-4.4-3-3.5 4.6-.1Z" /><path d="M18 16v5M15.5 18.5h5" /></>,
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
  const [audioCapabilities, setAudioCapabilities] = useState<AudioCapabilities>({ systemAudioAvailable: false, microphones: [] });
  const [systemAudio, setSystemAudio] = useState(localStorage.getItem("focusflow.systemAudio") !== "false");
  const [microphone, setMicrophone] = useState(localStorage.getItem("focusflow.microphone") === "true");
  const [microphoneDeviceId, setMicrophoneDeviceId] = useState(localStorage.getItem("focusflow.microphoneDevice") || "");
  const [customCursors, setCustomCursors] = useState(localStorage.getItem("focusflow.customCursors") === "true");
  const [cursorSize, setCursorSize] = useState(readCursorSize);
  const [cursorPaths, setCursorPaths] = useState<Partial<Record<CursorRole, string>>>(readCursorPaths);
  const [cursorPreviewRole, setCursorPreviewRole] = useState<CursorRole>("normal");
  const [cursorPreview, setCursorPreview] = useState<CursorPreview | null>(null);
  const [cursorPreviewError, setCursorPreviewError] = useState<string | null>(null);
  const [pageBackgroundPath, setPageBackgroundPath] = useState<string | null>(null);
  const [pageBackgroundSettings, setPageBackgroundSettings] = useState<PageBackgroundSettings>(readPageBackgroundSettings);
  const [pageBackgroundDraft, setPageBackgroundDraft] = useState<PageBackgroundSettings>(readPageBackgroundSettings);
  const [pageBackgroundDialogOpen, setPageBackgroundDialogOpen] = useState(false);
  const [applicationAspectRatio, setApplicationAspectRatio] = useState(16 / 9);
  const appFrameRef = useRef<HTMLElement | null>(null);
  const cropDragRef = useRef<{ x: number; y: number; positionX: number; positionY: number } | null>(null);
  const cursorPreviewRequestRef = useRef(0);
  const statusRef = useRef(status);
  const busyRef = useRef(busy);
  const shortcutRef = useRef(shortcut);
  const shortcutDialogRef = useRef(shortcutDialogOpen);
  const toggleRef = useRef<() => Promise<void>>(async () => undefined);
  const shortcutQueueRef = useRef<Promise<void>>(Promise.resolve());
  const resolvedLanguage = language === "system" ? systemLanguage() : language;
  const t = translations[resolvedLanguage];
  const audioText = audioTranslations[resolvedLanguage];
  const effectsText = effectsTranslations[resolvedLanguage];
  const pageBackgroundText = pageBackgroundTranslations[resolvedLanguage];

  useEffect(() => {
    void invoke<string | null>("page_background_path")
      .then(setPageBackgroundPath)
      .catch(() => setPageBackgroundPath(null));
  }, []);

  useEffect(() => {
    const element = appFrameRef.current;
    if (!element) return;
    const update = () => {
      const bounds = element.getBoundingClientRect();
      if (bounds.width > 0 && bounds.height > 0) setApplicationAspectRatio(bounds.width / bounds.height);
    };
    update();
    const observer = new ResizeObserver(update);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (activePage !== "effects") return;
    let cancelled = false;
    const requestId = ++cursorPreviewRequestRef.current;
    setCursorPreview(null);
    setCursorPreviewError(null);
    const customPath = customCursors ? cursorPaths[cursorPreviewRole] || null : null;
    const timer = window.setTimeout(() => {
      void invoke<CursorPreview>("render_cursor_preview", { role: cursorPreviewRole, customPath, sizePercent: cursorSize })
        .then((preview) => {
          if (!cancelled && requestId === cursorPreviewRequestRef.current) {
            setCursorPreview(preview);
            setCursorPreviewError(null);
          }
        })
        .catch((error) => {
          if (!cancelled && requestId === cursorPreviewRequestRef.current) {
            const description = String(error);
            setCursorPreviewError(description);
            setMessage(`指针预览失败：${description}`);
          }
        });
    }, 100);
    return () => { cancelled = true; window.clearTimeout(timer); };
  }, [activePage, customCursors, cursorPaths, cursorPreviewRole, cursorSize]);

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
        let savedDirectoryWarning: string | null = null;
        if (savedDirectory && savedDirectory !== next.outputDirectory) {
          try {
            next = await invoke<RecorderStatus>("set_output_directory", { directory: savedDirectory });
          } catch {
            localStorage.removeItem("focusflow.outputDirectory");
            savedDirectoryWarning = `之前的保存位置当前不可用，已自动切换到 ${next.outputDirectory}`;
          }
        }
        setStatus(next);
        setMessage(savedDirectoryWarning || (next.ffmpegAvailable ? "准备就绪" : "未找到 FFmpeg，暂时无法录制"));
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    }
    void initialize();
  }, []);

  useEffect(() => {
    let cancelled = false;
    void invoke<AudioCapabilities>("list_audio_devices")
      .catch(() => ({ systemAudioAvailable: false, microphones: [] }))
      .then((audio) => {
        if (cancelled) return;
        setAudioCapabilities(audio);
        const savedMicrophone = localStorage.getItem("focusflow.microphoneDevice");
        const selectedMicrophone = audio.microphones.find((device) => device.id === savedMicrophone)
          || audio.microphones.find((device) => device.isDefault)
          || audio.microphones[0];
        if (selectedMicrophone) setMicrophoneDeviceId(selectedMicrophone.id);
        if (!audio.systemAudioAvailable) setSystemAudio(false);
        if (audio.microphones.length === 0) setMicrophone(false);
      });
    return () => { cancelled = true; };
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
      setActivePage("record");
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
          audioOptions: {
            systemAudio: systemAudio && audioCapabilities.systemAudioAvailable,
            microphone: microphone && audioCapabilities.microphones.length > 0,
            microphoneDeviceId: microphoneDeviceId || null,
          },
          cursorOptions: {
            enabled: cursorSize !== 100 || (customCursors && Object.keys(cursorPaths).length > 0),
            customEnabled: customCursors,
            sizePercent: cursorSize,
            customPaths: cursorPaths,
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

  async function choosePageBackground() {
    try {
      const selected = await invoke<string | null>("choose_page_background");
      if (!selected) return;
      setPageBackgroundPath(selected);
      setPageBackgroundDraft(pageBackgroundSettings);
      setPageBackgroundDialogOpen(true);
    } catch (error) {
      setMessage(readableError(error));
    }
  }

  async function clearPageBackground() {
    try {
      await invoke("clear_page_background");
      setPageBackgroundPath(null);
      setPageBackgroundSettings(defaultPageBackgroundSettings);
      setPageBackgroundDraft(defaultPageBackgroundSettings);
      localStorage.removeItem("focusflow.pageBackgroundSettings");
      setMessage("应用已恢复默认外观");
    } catch (error) {
      setMessage(readableError(error));
    }
  }

  function savePageBackgroundSettings() {
    const next = pageBackgroundDraft.fit === "contain" ? { ...pageBackgroundDraft, zoom: 1 } : pageBackgroundDraft;
    setPageBackgroundSettings(next);
    localStorage.setItem("focusflow.pageBackgroundSettings", JSON.stringify(next));
    setPageBackgroundDialogOpen(false);
  }

  function movePageBackground(event: React.PointerEvent<HTMLDivElement>) {
    const drag = cropDragRef.current;
    if (!drag || pageBackgroundDraft.fit !== "cover") return;
    const bounds = event.currentTarget.getBoundingClientRect();
    const positionX = Math.min(100, Math.max(0, drag.positionX - (event.clientX - drag.x) / bounds.width * 100));
    const positionY = Math.min(100, Math.max(0, drag.positionY - (event.clientY - drag.y) / bounds.height * 100));
    setPageBackgroundDraft((current) => ({ ...current, positionX, positionY }));
  }

  function pageBackgroundImageStyle(settings: PageBackgroundSettings): React.CSSProperties {
    return {
      objectFit: settings.fit,
      objectPosition: `${settings.positionX}% ${settings.positionY}%`,
      transform: `scale(${settings.fit === "cover" ? settings.zoom : 1})`,
    };
  }

  async function chooseCursorFile(role: CursorRole) {
    if (status.isRecording || busy) return;
    const selected = await invoke<string | null>("choose_cursor_file", { initialDirectory: localStorage.getItem("focusflow.cursorDirectory") });
    if (!selected) return;
    const next = { ...cursorPaths, [role]: selected };
    setCursorPaths(next);
    setCustomCursors(true);
    setCursorPreviewRole(role);
    localStorage.setItem("focusflow.cursorPaths", JSON.stringify(next));
    localStorage.setItem("focusflow.customCursors", "true");
    localStorage.setItem("focusflow.cursorDirectory", selected.replace(/[\\/][^\\/]+$/, ""));
    setMessage(`已替换${cursorRoles.find(([id]) => id === role)?.[1] || role}`);
  }

  function restoreCursorSize() {
    setCursorSize(100);
    localStorage.setItem("focusflow.cursorSize", "100");
    setMessage("鼠标大小已恢复为 Windows 默认大小");
  }

  function restoreCursor(role: CursorRole) {
    const next = { ...cursorPaths };
    delete next[role];
    setCursorPaths(next);
    localStorage.setItem("focusflow.cursorPaths", JSON.stringify(next));
    setMessage("这个角色已恢复为 Windows 默认指针");
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
      <section ref={appFrameRef} className={`app-frame product-shell ${pageBackgroundPath ? "has-custom-page-background" : ""}`}>
        {pageBackgroundPath && <div className="custom-page-background"><img src={`${convertFileSrc(pageBackgroundPath)}?page-background=1`} alt="" style={pageBackgroundImageStyle(pageBackgroundSettings)} /></div>}
        <aside className="side-rail">
          <button className={`rail-brand ${activePage === "record" ? "active" : ""}`} type="button" aria-label={t.record} onClick={() => setActivePage("record")}><span className="brand-mark"><i /></span></button>
          <nav className="rail-navigation" aria-label="BreadShot">
            <button type="button" className={`rail-button ${activePage === "effects" ? "active" : ""}`} title={effectsText.nav} aria-label={effectsText.nav} aria-current={activePage === "effects" ? "page" : undefined} onClick={() => setActivePage("effects")}><Glyph name="effects" /><span>{effectsText.nav}</span></button>
            {(["settings", "language", "update"] as const).map((page) => <button key={page} type="button" className={`rail-button ${activePage === page ? "active" : ""}`} title={t[page]} aria-label={t[page]} aria-current={activePage === page ? "page" : undefined} onClick={() => setActivePage(page)}><Glyph name={page} /><span>{t[page]}</span></button>)}
          </nav>
          <span className="rail-version">v{APP_VERSION}</span>
        </aside>

        <section className="app-main">
          <header className="main-header">
            <div className="brand-copy"><strong>BreadShot</strong><small>ZERO-EDIT SCREEN RECORDER</small></div>
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
                <div className="audio-status-row"><span className={systemAudio ? "enabled" : ""}>◉ {audioText.system} · {systemAudio ? audioText.on : audioText.off}</span><span className={microphone ? "enabled" : ""}>⌁ {audioText.microphone} · {microphone ? audioText.on : audioText.off}</span></div>
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
                  <div className="setting-row"><span className="setting-index">04</span><div className="setting-copy"><strong>{audioText.system}</strong><span>{audioCapabilities.systemAudioAvailable ? audioText.systemHelp : audioText.unavailable}</span></div><button type="button" role="switch" aria-checked={systemAudio} className={`toggle-switch ${systemAudio ? "active" : ""}`} disabled={!audioCapabilities.systemAudioAvailable || status.isRecording || busy} onClick={() => { const next = !systemAudio; setSystemAudio(next); localStorage.setItem("focusflow.systemAudio", String(next)); }}><i /></button></div>
                  <div className="setting-row"><span className="setting-index">05</span><div className="setting-copy"><strong>{audioText.microphone}</strong><span>{audioCapabilities.microphones.length > 0 ? audioText.microphoneHelp : audioText.noMicrophone}</span></div><div className="audio-control"><select className="display-select microphone-select" value={microphoneDeviceId} disabled={!microphone || status.isRecording || busy || audioCapabilities.microphones.length === 0} onChange={(event) => { setMicrophoneDeviceId(event.target.value); localStorage.setItem("focusflow.microphoneDevice", event.target.value); }}>{audioCapabilities.microphones.map((device) => <option key={device.id} value={device.id}>{device.name}{device.isDefault ? ` · ${audioText.defaultDevice}` : ""}</option>)}</select><button type="button" role="switch" aria-checked={microphone} className={`toggle-switch ${microphone ? "active" : ""}`} disabled={audioCapabilities.microphones.length === 0 || status.isRecording || busy} onClick={() => { const next = !microphone; setMicrophone(next); localStorage.setItem("focusflow.microphone", String(next)); }}><i /></button></div></div>
                  <div className="setting-row"><span className="setting-index">06</span><div className="setting-copy"><strong>{pageBackgroundText.title}</strong><span>{pageBackgroundText.help}</span></div><div className="page-background-actions"><button type="button" className="secondary-button" onClick={() => void choosePageBackground()}>{pageBackgroundText.choose}</button>{pageBackgroundPath && <><button type="button" className="secondary-button" onClick={() => { setPageBackgroundDraft(pageBackgroundSettings); setPageBackgroundDialogOpen(true); }}>{pageBackgroundText.adjust}</button><button type="button" className="secondary-button danger-subtle" onClick={() => void clearPageBackground()}>{pageBackgroundText.restore}</button></>}</div></div>
                  <div className="privacy-note"><span>⌾</span><div><strong>{t.privacy}</strong><small>{t.privacyHelp}</small></div></div>
                </div>
              </section>
            )}

            {activePage === "effects" && (
              <section className="content-page effects-page">
                <div className="page-heading"><span className="section-kicker">02 / EFFECTS</span><h1>{effectsText.title}</h1><p>{effectsText.lead}</p></div>
                <div className="effects-layout">
                  <section className="cursor-config-card">
                    <div className="effects-card-heading"><div><strong>{effectsText.custom}</strong><span>{effectsText.customHelp}</span></div><button type="button" role="switch" aria-checked={customCursors} className={`toggle-switch ${customCursors ? "active" : ""}`} disabled={status.isRecording || busy} onClick={() => { const next = !customCursors; setCustomCursors(next); localStorage.setItem("focusflow.customCursors", String(next)); }}><i /></button></div>
                    <div className="cursor-role-grid">
                      {cursorRoles.map(([role, label]) => <button key={role} type="button" className={`cursor-role ${cursorPreviewRole === role ? "active" : ""}`} onClick={() => setCursorPreviewRole(role)}><span>{label}</span><small>{customCursors && cursorPaths[role] ? fileName(cursorPaths[role]!) : effectsText.windowsDefault}</small></button>)}
                    </div>
                    <div className="cursor-role-actions"><button type="button" className="primary-button compact" disabled={status.isRecording || busy} onClick={() => void chooseCursorFile(cursorPreviewRole)}>{effectsText.choose}</button><button type="button" className="secondary-button" disabled={!cursorPaths[cursorPreviewRole] || status.isRecording || busy} onClick={() => restoreCursor(cursorPreviewRole)}>{effectsText.restore}</button></div>
                  </section>
                  <section className="cursor-preview-card">
                    <div className="effects-card-heading"><div><strong>{effectsText.size}</strong><span>{effectsText.sizeHelp}</span></div><b>{cursorSize}%</b></div>
                    <div className="cursor-preview-stage">
                      {cursorPreview && <img key={cursorPreview.path} src={`${convertFileSrc(cursorPreview.path)}?v=${encodeURIComponent(cursorPreview.path)}`} alt={cursorRoles.find(([role]) => role === cursorPreviewRole)?.[1]} />}
                      {cursorPreviewError && <span className="cursor-preview-error">自定义指针文件当前不可访问<br /><small>{cursorPreviewError}</small></span>}
                    </div>
                    <label className="cursor-size-slider"><span>50%</span><input type="range" min="50" max="250" step="5" value={cursorSize} disabled={status.isRecording || busy} onChange={(event) => { const next = Number(event.target.value); setCursorSize(next); localStorage.setItem("focusflow.cursorSize", String(next)); }} /><span>250%</span></label>
                    <div className="cursor-preview-controls"><label className="cursor-preview-select"><span>{effectsText.preview}</span><select className="display-select" value={cursorPreviewRole} onChange={(event) => setCursorPreviewRole(event.target.value as CursorRole)}>{cursorRoles.map(([role, label]) => <option key={role} value={role}>{label}</option>)}</select></label><button type="button" className="secondary-button reset-size-button" disabled={cursorSize === 100 || status.isRecording || busy} onClick={restoreCursorSize}>{effectsText.resetSize}</button></div>
                  </section>
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
                <div className="version-card"><div className="version-icon"><Glyph name="update" /></div><div><span>{t.currentVersion}</span><strong>BreadShot v{APP_VERSION}</strong><small>{t.latestLocal}</small></div><b>{t.current}</b></div>
                <p className="coming-soon"><span />{t.updateSoon}</p>
              </section>
            )}
          </div>

          <footer><span>BreadShot Alpha · Built for Windows</span><div><span>LOCAL PROCESSING</span><i /> <span>H.264 MP4</span><i /> <span>NO WATERMARK</span></div></footer>
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

      {pageBackgroundDialogOpen && pageBackgroundPath && (
        <div className="modal-backdrop" role="presentation" onMouseDown={() => setPageBackgroundDialogOpen(false)}>
          <section className="page-background-modal" role="dialog" aria-modal="true" aria-labelledby="page-background-title" onMouseDown={(event) => event.stopPropagation()}>
            <div className="page-background-modal-heading"><div><p className="eyebrow">APPEARANCE</p><h2 id="page-background-title">{pageBackgroundText.modal}</h2></div><button type="button" className="modal-close" aria-label={pageBackgroundText.cancel} onClick={() => setPageBackgroundDialogOpen(false)}>×</button></div>
            <div className="page-background-modes"><button type="button" className={pageBackgroundDraft.fit === "cover" ? "active" : ""} onClick={() => setPageBackgroundDraft((current) => ({ ...current, fit: "cover" }))}>{pageBackgroundText.fill}</button><button type="button" className={pageBackgroundDraft.fit === "contain" ? "active" : ""} onClick={() => setPageBackgroundDraft((current) => ({ ...current, fit: "contain", zoom: 1 }))}>{pageBackgroundText.full}</button></div>
            <p className="page-background-help">{pageBackgroundDraft.fit === "cover" ? pageBackgroundText.fillHelp : pageBackgroundText.fullHelp}</p>
            <div className={`page-background-crop ${pageBackgroundDraft.fit}`} style={{ aspectRatio: String(applicationAspectRatio) }} onPointerDown={(event) => { if (pageBackgroundDraft.fit !== "cover") return; event.currentTarget.setPointerCapture(event.pointerId); cropDragRef.current = { x: event.clientX, y: event.clientY, positionX: pageBackgroundDraft.positionX, positionY: pageBackgroundDraft.positionY }; }} onPointerMove={movePageBackground} onPointerUp={() => { cropDragRef.current = null; }} onPointerCancel={() => { cropDragRef.current = null; }} onWheel={(event) => { if (pageBackgroundDraft.fit !== "cover") return; event.preventDefault(); const zoom = Math.min(2.5, Math.max(1, pageBackgroundDraft.zoom - event.deltaY * 0.001)); setPageBackgroundDraft((current) => ({ ...current, zoom })); }}><img src={`${convertFileSrc(pageBackgroundPath)}?crop-preview=1`} alt="" draggable={false} style={pageBackgroundImageStyle(pageBackgroundDraft)} /><span className="crop-grid" /></div>
            <label className={`page-background-zoom ${pageBackgroundDraft.fit === "contain" ? "disabled" : ""}`}><span>{pageBackgroundText.zoom}</span><input type="range" min="1" max="2.5" step="0.01" disabled={pageBackgroundDraft.fit === "contain"} value={pageBackgroundDraft.fit === "contain" ? 1 : pageBackgroundDraft.zoom} onChange={(event) => setPageBackgroundDraft((current) => ({ ...current, zoom: Number(event.target.value) }))} /><b>{Math.round((pageBackgroundDraft.fit === "contain" ? 1 : pageBackgroundDraft.zoom) * 100)}%</b></label>
            <div className="modal-actions"><button type="button" className="secondary-button" onClick={() => setPageBackgroundDialogOpen(false)}>{pageBackgroundText.cancel}</button><button type="button" className="primary-button compact" onClick={savePageBackgroundSettings}>{pageBackgroundText.save}</button></div>
          </section>
        </div>
      )}
    </main>
  );
}

ReactDOM.createRoot(document.getElementById("root")!).render(<React.StrictMode><App /></React.StrictMode>);
