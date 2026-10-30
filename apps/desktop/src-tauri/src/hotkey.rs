use focusflow_recorder::single_key_code;
use std::str::FromStr;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Shortcut};

pub enum HotkeyBindError {
    /// The previous registration was left in place.
    Unchanged(String),
    /// The previous registration was released. `restored` means it was registered again.
    NotRegistered { message: String, restored: bool },
}

impl HotkeyBindError {
    pub fn message(&self) -> &str {
        match self {
            Self::Unchanged(message) | Self::NotRegistered { message, .. } => message,
        }
    }
}

pub fn hotkey_failure_message(shortcut: &str) -> String {
    format!("快捷键 {shortcut} 无法注册，可能已被其他程序占用。所有按钮仍可点击，请更换快捷键。")
}

fn code_from_user(raw: &str) -> Result<Code, String> {
    let name = single_key_code(raw)?;
    Code::from_str(&name).map_err(|_| format!("不支持的快捷键：{raw}"))
}

fn restore(app: &AppHandle, previous: Option<&str>) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let Ok(code) = code_from_user(previous) else {
        return false;
    };
    let shortcut = Shortcut::new(None, code);
    let manager = app.global_shortcut();
    manager.register(shortcut).is_ok() && manager.is_registered(shortcut)
}

pub fn bind_global_hotkey(
    app: &AppHandle,
    shortcut: &str,
    previous: Option<&str>,
) -> Result<(), HotkeyBindError> {
    let code = code_from_user(shortcut).map_err(HotkeyBindError::Unchanged)?;
    let parsed = Shortcut::new(None, code);
    let manager = app.global_shortcut();
    if manager.unregister_all().is_err() {
        let restored = restore(app, previous);
        return Err(HotkeyBindError::NotRegistered {
            message: hotkey_failure_message(shortcut),
            restored,
        });
    }
    if manager.register(parsed).is_ok() && manager.is_registered(parsed) {
        return Ok(());
    }
    let restored = restore(app, previous);
    Err(HotkeyBindError::NotRegistered {
        message: hotkey_failure_message(shortcut),
        restored,
    })
}
