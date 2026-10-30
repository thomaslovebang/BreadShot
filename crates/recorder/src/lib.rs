#![doc = "Recording session lifecycle and capture contracts."]

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RecordingState {
    #[default]
    Idle,
    Recording,
    Stopping,
    Completed,
    Failed,
}

/// Region passed to `FFmpeg` `gdigrab`. Offsets are absolute virtual-screen pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GdigrabRegion {
    pub offset_x: i32,
    pub offset_y: i32,
    pub width: u32,
    pub height: u32,
}

/// `libx264` with `yuv420p` needs even dimensions. Returns `None` when the area collapses.
#[must_use]
pub fn gdigrab_region(x: i32, y: i32, width: u32, height: u32) -> Option<GdigrabRegion> {
    let width = width - (width % 2);
    let height = height - (height % 2);
    if width == 0 || height == 0 {
        return None;
    }
    Some(GdigrabRegion {
        offset_x: x,
        offset_y: y,
        width,
        height,
    })
}

/// Simplified-Chinese label for one monitor in the capture picker.
#[must_use]
pub fn capture_display_label(
    index: usize,
    name: &str,
    width: u32,
    height: u32,
    primary: bool,
) -> String {
    let number = index + 1;
    let role = if primary { "主屏幕" } else { "扩展屏幕" };
    let short_name = short_display_name(name);
    if short_name.is_empty() {
        format!("显示器 {number}（{role}，{width}×{height}）")
    } else {
        format!("显示器 {number}：{short_name}（{role}，{width}×{height}）")
    }
}

fn short_display_name(name: &str) -> String {
    name.trim()
        .strip_prefix("\\\\.\\")
        .unwrap_or(name.trim())
        .trim()
        .to_string()
}

/// Pick a monitor for capture.
///
/// `None` means the whole virtual desktop. A requested name that is no longer
/// present is an error, so a stale index cannot silently record a different screen.
///
/// # Errors
///
/// Returns an error when the requested monitor is disconnected or ambiguous.
pub fn resolve_display_selection(
    names: &[&str],
    index: Option<usize>,
    name: Option<&str>,
) -> Result<Option<usize>, &'static str> {
    let name = name.map(str::trim).filter(|value| !value.is_empty());
    if index.is_none() && name.is_none() {
        return Ok(None);
    }
    if let Some(name) = name {
        let matches = names
            .iter()
            .enumerate()
            .filter_map(|(candidate_index, candidate)| {
                (*candidate == name).then_some(candidate_index)
            })
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [only] => Ok(Some(*only)),
            several if !several.is_empty() => index
                .filter(|value| several.contains(value))
                .map(Some)
                .ok_or("所选显示器不明确，请重新选择。"),
            _ => Err("所选显示器已断开，请重新选择。"),
        };
    }
    index
        .filter(|value| *value < names.len())
        .map(Some)
        .ok_or("所选显示器已断开，请重新选择。")
}

/// Map one user hotkey (`F8`, `G`, `1`) to a `keyboard_types` code name.
///
/// # Errors
///
/// Returns an error when `raw` is not one letter, digit, or an `F1`–`F24` key.
pub fn single_key_code(raw: &str) -> Result<String, String> {
    let key = raw.trim().to_ascii_uppercase();
    if let Some(rest) = key.strip_prefix('F') {
        if rest.chars().all(|character| character.is_ascii_digit()) && !rest.starts_with('0') {
            if let Ok(number) = rest.parse::<u8>() {
                if (1..=24).contains(&number) {
                    return Ok(format!("F{number}"));
                }
            }
        }
    }
    let mut characters = key.chars();
    let Some(character) = characters.next() else {
        return Err("快捷键不能为空".to_string());
    };
    if characters.next().is_some() {
        return Err(format!("不支持的快捷键：{key}"));
    }
    if character.is_ascii_uppercase() {
        return Ok(format!("Key{character}"));
    }
    if character.is_ascii_digit() {
        return Ok(format!("Digit{character}"));
    }
    Err(format!("不支持的快捷键：{key}"))
}

#[cfg(test)]
mod tests {
    use super::{
        capture_display_label, gdigrab_region, resolve_display_selection, single_key_code,
    };

    #[test]
    fn gdigrab_uses_absolute_offsets_and_even_sizes() {
        let region = gdigrab_region(-1920, 120, 1920, 1080).expect("region");
        assert_eq!(region.offset_x, -1920);
        assert_eq!(region.offset_y, 120);
        assert_eq!((region.width, region.height), (1920, 1080));

        let odd = gdigrab_region(0, 0, 1921, 1081).expect("odd region");
        assert_eq!((odd.width, odd.height), (1920, 1080));
        assert!(gdigrab_region(0, 0, 1, 80).is_none());
    }

    #[test]
    fn display_labels_are_simplified_chinese() {
        assert_eq!(
            capture_display_label(0, r"\\.\DISPLAY1", 1920, 1080, true),
            "显示器 1：DISPLAY1（主屏幕，1920×1080）"
        );
        assert_eq!(
            capture_display_label(1, "DELL U2720Q", 2560, 1440, false),
            "显示器 2：DELL U2720Q（扩展屏幕，2560×1440）"
        );
        assert_eq!(
            capture_display_label(2, "  ", 1280, 720, false),
            "显示器 3（扩展屏幕，1280×720）"
        );
    }

    #[test]
    fn display_selection_prefers_name_and_rejects_missing_monitors() {
        let names = ["\\\\.\\DISPLAY1", "\\\\.\\DISPLAY2"];
        assert_eq!(resolve_display_selection(&names, None, None), Ok(None));
        assert_eq!(
            resolve_display_selection(&names, Some(0), Some(r"\\.\DISPLAY2")),
            Ok(Some(1))
        );
        assert_eq!(
            resolve_display_selection(&names, Some(9), None),
            Err("所选显示器已断开，请重新选择。")
        );
        assert_eq!(
            resolve_display_selection(&names, Some(0), Some("MISSING")),
            Err("所选显示器已断开，请重新选择。")
        );
    }

    #[test]
    fn single_keys_map_to_keyboard_codes() {
        assert_eq!(single_key_code("F8").expect("f8"), "F8");
        assert_eq!(single_key_code("g").expect("g"), "KeyG");
        assert_eq!(single_key_code("1").expect("1"), "Digit1");
        assert!(single_key_code("F08").is_err());
        assert!(single_key_code("Ctrl").is_err());
    }
}
