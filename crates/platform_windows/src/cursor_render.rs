use image::{
    Delay, Frame, ImageFormat, Rgba, RgbaImage,
    codecs::gif::{GifEncoder, Repeat},
    imageops::FilterType,
};
use std::{os::windows::ffi::OsStrExt, path::Path};
use windows::{
    Win32::{
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACKNESS, CreateCompatibleDC,
            CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, HGDIOBJ, PatBlt,
            SelectObject, WHITENESS,
        },
        System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
        UI::WindowsAndMessaging::{
            DI_NORMAL, DrawIconEx, GetIconInfo, GetSystemMetrics, HCURSOR, HICON, ICONINFO,
            LoadCursorFromFileW, LoadCursorW, SM_CXCURSOR,
        },
    },
    core::{PCWSTR, w},
};

pub const CURSOR_ROLES: [(&str, u16); 17] = [
    ("normal", 32512),
    ("help", 32651),
    ("app_starting", 32650),
    ("wait", 32514),
    ("crosshair", 32515),
    ("text", 32513),
    ("handwriting", 32631),
    ("unavailable", 32648),
    ("vertical_resize", 32645),
    ("horizontal_resize", 32644),
    ("diagonal_resize_1", 32642),
    ("diagonal_resize_2", 32643),
    ("move", 32646),
    ("alternate", 32516),
    ("link", 32649),
    ("location", 32671),
    ("person", 32672),
];

#[derive(Debug, Clone, Copy)]
pub struct CursorImageInfo {
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct CursorAnimationFrame {
    pub image: RgbaImage,
    pub duration_ms: u32,
}

#[derive(Debug, Clone)]
pub struct CursorAnimation {
    pub frames: Vec<CursorAnimationFrame>,
    pub hotspot_x: u32,
    pub hotspot_y: u32,
    pub width: u32,
    pub height: u32,
}

pub fn role_id(role: &str) -> Option<u16> {
    CURSOR_ROLES
        .iter()
        .find_map(|(name, id)| (*name == role).then_some(*id))
}

pub fn render_cursor_png(
    role: &str,
    custom_path: Option<&Path>,
    size_percent: u32,
    output: &Path,
) -> Result<CursorImageInfo, String> {
    if custom_path.is_some_and(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ani"))) {
        let animation = render_cursor_animation(role, custom_path, size_percent)?;
        animation.frames[0].image.save(output)
            .map_err(|error| format!("无法保存鼠标指针预览：{error}"))?;
        return Ok(CursorImageInfo { hotspot_x: animation.hotspot_x, hotspot_y: animation.hotspot_y, width: animation.width, height: animation.height });
    }
    let cursor = load_cursor(role, custom_path)?;
    let scale = size_percent.clamp(50, 250) as f64 / 100.0;
    let windows_size = windows_cursor_base_size();
    let size = (f64::from(windows_size) * scale).round().clamp(8.0, 256.0) as i32;
    let mut icon_info = ICONINFO::default();
    unsafe { GetIconInfo(HICON(cursor.0), &mut icon_info) }
        .map_err(|error| format!("无法读取鼠标指针热点：{error}"))?;
    let hotspot_x = (f64::from(icon_info.xHotspot) * scale).round() as u32;
    let hotspot_y = (f64::from(icon_info.yHotspot) * scale).round() as u32;
    let black = draw_cursor(cursor, size, false)?;
    let white = draw_cursor(cursor, size, true)?;
    if !icon_info.hbmColor.is_invalid() {
        let _ = unsafe { DeleteObject(HGDIOBJ(icon_info.hbmColor.0)) };
    }
    if !icon_info.hbmMask.is_invalid() {
        let _ = unsafe { DeleteObject(HGDIOBJ(icon_info.hbmMask.0)) };
    }
    let mut image = RgbaImage::new(size as u32, size as u32);
    for y in 0..size as u32 {
        for x in 0..size as u32 {
            let offset = ((y * size as u32 + x) * 4) as usize;
            let black_rgb = [black[offset + 2], black[offset + 1], black[offset]];
            let white_rgb = [white[offset + 2], white[offset + 1], white[offset]];
            let background = (0..3)
                .map(|channel| white_rgb[channel].saturating_sub(black_rgb[channel]) as u32)
                .sum::<u32>()
                / 3;
            let alpha = 255_u32.saturating_sub(background) as u8;
            let color = if alpha == 0 {
                [0, 0, 0]
            } else {
                [
                    ((u32::from(black_rgb[0]) * 255) / u32::from(alpha)).min(255) as u8,
                    ((u32::from(black_rgb[1]) * 255) / u32::from(alpha)).min(255) as u8,
                    ((u32::from(black_rgb[2]) * 255) / u32::from(alpha)).min(255) as u8,
                ]
            };
            image.put_pixel(x, y, Rgba([color[0], color[1], color[2], alpha]));
        }
    }
    image
        .save(output)
        .map_err(|error| format!("无法保存鼠标指针预览：{error}"))?;
    Ok(CursorImageInfo {
        hotspot_x,
        hotspot_y,
        width: size as u32,
        height: size as u32,
    })
}

pub fn render_cursor_animation(
    role: &str,
    custom_path: Option<&Path>,
    size_percent: u32,
) -> Result<CursorAnimation, String> {
    if let Some(path) = custom_path.filter(|path| path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("ani"))) {
        return decode_ani(path, size_percent);
    }
    let temp = std::env::temp_dir().join(format!("focusflow-cursor-{}-{}.png", std::process::id(), role));
    let info = render_cursor_png(role, custom_path, size_percent, &temp)?;
    let image = image::open(&temp).map_err(|error| format!("无法读取鼠标指针帧：{error}"))?.into_rgba8();
    let _ = std::fs::remove_file(temp);
    Ok(CursorAnimation {
        frames: vec![CursorAnimationFrame { image, duration_ms: 1000 }],
        hotspot_x: info.hotspot_x,
        hotspot_y: info.hotspot_y,
        width: info.width,
        height: info.height,
    })
}

pub fn save_cursor_preview(animation: &CursorAnimation, output: &Path) -> Result<(), String> {
    let (left, top, right, bottom) = animation.frames.iter().fold(
        (animation.width, animation.height, 0_u32, 0_u32),
        |bounds, frame| {
            let mut next = bounds;
            for (x, y, pixel) in frame.image.enumerate_pixels() {
                if pixel[3] > 4 {
                    next.0 = next.0.min(x); next.1 = next.1.min(y);
                    next.2 = next.2.max(x + 1); next.3 = next.3.max(y + 1);
                }
            }
            next
        },
    );
    let has_content = right > left && bottom > top;
    let preview_frames = animation.frames.iter().map(|frame| CursorAnimationFrame {
        image: if has_content {
            image::imageops::crop_imm(&frame.image, left, top, right - left, bottom - top).to_image()
        } else { frame.image.clone() },
        duration_ms: frame.duration_ms,
    }).collect::<Vec<_>>();
    if animation.frames.len() == 1 {
        preview_frames[0].image.save(output)
            .map_err(|error| format!("无法保存鼠标指针预览：{error}"))?;
        return Ok(());
    }
    let file = std::fs::File::create(output).map_err(|error| format!("无法创建 ANI 预览：{error}"))?;
    let mut encoder = GifEncoder::new(file);
    encoder.set_repeat(Repeat::Infinite).map_err(|error| format!("无法设置 ANI 循环：{error}"))?;
    for frame in &preview_frames {
        encoder.encode_frame(Frame::from_parts(
            frame.image.clone(), 0, 0,
            Delay::from_numer_denom_ms(frame.duration_ms.max(16), 1),
        )).map_err(|error| format!("无法编码 ANI 预览：{error}"))?;
    }
    Ok(())
}

fn decode_ani(path: &Path, size_percent: u32) -> Result<CursorAnimation, String> {
    let data = std::fs::read(path).map_err(|error| format!("无法读取 ANI 文件：{error}"))?;
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"ACON" {
        return Err("ANI 文件结构无效".to_string());
    }
    let mut icon_chunks = Vec::<Vec<u8>>::new();
    let mut rates = Vec::<u32>::new();
    let mut sequence = Vec::<u32>::new();
    let mut default_rate = 6_u32;
    parse_ani_chunks(&data[12..], &mut icon_chunks, &mut rates, &mut sequence, &mut default_rate)?;
    if icon_chunks.is_empty() { return Err("ANI 文件中没有可用帧".to_string()); }
    if sequence.is_empty() { sequence = (0..icon_chunks.len() as u32).collect(); }
    let windows_size = windows_cursor_base_size();
    let scale = size_percent.clamp(50, 250) as f64 / 100.0;
    let size = (f64::from(windows_size) * scale).round().clamp(8.0, 256.0) as u32;
    let first = &icon_chunks[sequence[0] as usize % icon_chunks.len()];
    let (raw_hotspot_x, raw_hotspot_y) = cursor_hotspot(first);
    let mut frames = Vec::with_capacity(sequence.len());
    for (step, frame_index) in sequence.iter().enumerate() {
        let bytes = &icon_chunks[*frame_index as usize % icon_chunks.len()];
        let decoded = image::load_from_memory_with_format(bytes, ImageFormat::Ico)
            .map_err(|error| format!("无法解码 ANI 帧：{error}"))?.into_rgba8();
        let resized = image::imageops::resize(&decoded, size, size, FilterType::Lanczos3);
        let jiffies = rates.get(step).copied().unwrap_or(default_rate).max(1);
        frames.push(CursorAnimationFrame {
            image: resized,
            duration_ms: ((u64::from(jiffies) * 1000 + 30) / 60) as u32,
        });
    }
    Ok(CursorAnimation {
        frames,
        hotspot_x: (f64::from(raw_hotspot_x) * scale).round() as u32,
        hotspot_y: (f64::from(raw_hotspot_y) * scale).round() as u32,
        width: size,
        height: size,
    })
}

fn parse_ani_chunks(
    mut bytes: &[u8], icons: &mut Vec<Vec<u8>>, rates: &mut Vec<u32>, sequence: &mut Vec<u32>, default_rate: &mut u32,
) -> Result<(), String> {
    while bytes.len() >= 8 {
        let id = &bytes[0..4];
        let length = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
        if bytes.len() < 8 + length { return Err("ANI 文件数据不完整".to_string()); }
        let payload = &bytes[8..8 + length];
        match id {
            b"anih" if payload.len() >= 32 => *default_rate = u32::from_le_bytes(payload[28..32].try_into().unwrap()).max(1),
            b"rate" => *rates = payload.chunks_exact(4).map(|v| u32::from_le_bytes(v.try_into().unwrap())).collect(),
            b"seq " => *sequence = payload.chunks_exact(4).map(|v| u32::from_le_bytes(v.try_into().unwrap())).collect(),
            b"icon" => icons.push(payload.to_vec()),
            b"LIST" if payload.len() >= 4 => parse_ani_chunks(&payload[4..], icons, rates, sequence, default_rate)?,
            _ => {}
        }
        let consumed = 8 + length + (length & 1);
        if consumed > bytes.len() { break; }
        bytes = &bytes[consumed..];
    }
    Ok(())
}

fn cursor_hotspot(bytes: &[u8]) -> (u16, u16) {
    if bytes.len() >= 14 && u16::from_le_bytes([bytes[2], bytes[3]]) == 2 {
        (u16::from_le_bytes([bytes[10], bytes[11]]), u16::from_le_bytes([bytes[12], bytes[13]]))
    } else { (0, 0) }
}

fn windows_cursor_base_size() -> i32 {
    let mut value = 0_u32;
    let mut bytes = std::mem::size_of::<u32>() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Control Panel\\Cursors"),
            w!("CursorBaseSize"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut bytes),
        )
    };
    if result.is_ok() && value >= 16 {
        value.min(256) as i32
    } else {
        unsafe { GetSystemMetrics(SM_CXCURSOR) }.max(16)
    }
}

fn load_cursor(role: &str, custom_path: Option<&Path>) -> Result<HCURSOR, String> {
    if let Some(path) = custom_path {
        let wide = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        return unsafe { LoadCursorFromFileW(PCWSTR(wide.as_ptr())) }
            .map_err(|error| format!("无法加载 CUR/ANI 指针：{error}"));
    }
    let id = role_id(role).ok_or_else(|| format!("未知鼠标指针角色：{role}"))?;
    unsafe { LoadCursorW(None, PCWSTR(usize::from(id) as *const u16)) }
        .map_err(|error| format!("无法加载 Windows 默认指针：{error}"))
}

fn draw_cursor(cursor: HCURSOR, size: i32, white_background: bool) -> Result<Vec<u8>, String> {
    let hdc = unsafe { CreateCompatibleDC(None) };
    if hdc.is_invalid() {
        return Err("无法创建指针画布".to_string());
    }
    let mut info = BITMAPINFO::default();
    info.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: size,
        biHeight: -size,
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB.0,
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let bitmap = unsafe {
        CreateDIBSection(Some(hdc), &info, DIB_RGB_COLORS, &mut bits, None, 0)
    }
    .map_err(|error| format!("无法创建指针位图：{error}"))?;
    let previous = unsafe { SelectObject(hdc, HGDIOBJ(bitmap.0)) };
    let raster = if white_background { WHITENESS } else { BLACKNESS };
    let _ = unsafe { PatBlt(hdc, 0, 0, size, size, raster) };
    unsafe {
        DrawIconEx(
            hdc,
            0,
            0,
            HICON(cursor.0),
            size,
            size,
            0,
            None,
            DI_NORMAL,
        )
    }
    .map_err(|error| format!("无法绘制鼠标指针：{error}"))?;
    let bytes = unsafe {
        std::slice::from_raw_parts(bits.cast::<u8>(), (size * size * 4) as usize).to_vec()
    };
    unsafe {
        SelectObject(hdc, previous);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(hdc);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_cursor_roles_have_unique_ids() {
        let mut ids = CURSOR_ROLES.iter().map(|(_, id)| *id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), CURSOR_ROLES.len());
    }

    #[test]
    fn renders_windows_default_cursor_to_png() {
        let path = std::env::temp_dir().join("focusflow-cursor-preview-test.png");
        let info = render_cursor_png("normal", None, 100, &path).unwrap();
        assert!(path.metadata().unwrap().len() > 100);
        assert!(info.width >= 16);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn decodes_windows_ani_with_timing() {
        let source = Path::new(r"C:\Windows\Cursors\aero_busy.ani");
        if !source.is_file() { return; }
        let animation = render_cursor_animation("wait", Some(source), 100).unwrap();
        assert!(animation.frames.len() > 1);
        assert!(animation.frames.iter().all(|frame| frame.duration_ms > 0));
        let preview = std::env::temp_dir().join("focusflow-ani-preview-test.gif");
        save_cursor_preview(&animation, &preview).unwrap();
        assert!(preview.metadata().unwrap().len() > 100);
        let _ = std::fs::remove_file(preview);
    }
}
