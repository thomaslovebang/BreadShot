export type CaptureDisplay = {
  index: number;
  name: string;
  label: string;
  x: number;
  y: number;
  width: number;
  height: number;
  primary: boolean;
};

export type DisplayChoice = { index: number; name: string } | "all";

export type TauriCallbackBridge = {
  transformCallback?: unknown;
  invoke?: unknown;
};

declare global {
  interface Window {
    __TAURI_INTERNALS__?: TauriCallbackBridge;
  }
}

export function errorText(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return `${error.name}: ${error.message}`;
  if (error && typeof error === "object" && "message" in error) {
    const message = error.message;
    if (typeof message === "string") return message;
  }
  return String(error ?? "");
}

export function isTransformCallbackFailure(error: unknown): boolean {
  return errorText(error).includes("transformCallback");
}

export function tauriCallbackBridgeReady(bridge: TauriCallbackBridge | null | undefined): boolean {
  return typeof bridge?.transformCallback === "function" && typeof bridge.invoke === "function";
}

export function hotkeyRegistrationMessage(shortcut: string, error: unknown): string {
  const detail = errorText(error).replace(/^(?:Error:\s*)+/i, "").trim();
  if (detail.includes("transformCallback")) {
    return `快捷键 ${shortcut} 注册失败，界面接口异常。所有按钮仍可点击，请更换快捷键。`;
  }
  if (detail.includes("所有按钮仍可点击")) return detail;
  return `快捷键 ${shortcut} 无法注册，可能已被其他程序占用。所有按钮仍可点击，请更换快捷键。`;
}

export function choiceToStorage(choice: DisplayChoice): string {
  return choice === "all" ? "all" : `${choice.index}|${choice.name}`;
}

export function choiceFromStorage(saved: string | null, displays: CaptureDisplay[]): DisplayChoice {
  if (saved === "all") return "all";
  if (saved) {
    const separator = saved.indexOf("|");
    if (separator >= 0) {
      const index = Number(saved.slice(0, separator));
      const name = saved.slice(separator + 1);
      const exact = displays.find((display) => display.index === index && display.name === name);
      if (exact) return { index: exact.index, name: exact.name };
      if (name) {
        const byName = displays.filter((display) => display.name === name);
        if (byName.length === 1) return { index: byName[0].index, name: byName[0].name };
      }
    }
  }
  const primary = displays.find((display) => display.primary) ?? displays[0];
  if (primary) return { index: primary.index, name: primary.name };
  return "all";
}

export function startDisplayArgs(choice: DisplayChoice): { displayIndex: number | null; displayName: string | null } {
  if (choice === "all") return { displayIndex: null, displayName: null };
  return { displayIndex: choice.index, displayName: choice.name || null };
}
