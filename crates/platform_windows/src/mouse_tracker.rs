use serde::Serialize;
use std::{
    fs::File,
    io::{BufWriter, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
    sync::OnceLock,
};
use windows::Win32::{
    Foundation::POINT,
    UI::{
        Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON},
        WindowsAndMessaging::{CURSORINFO, CURSOR_SHOWING, GetCursorInfo, GetCursorPos, LoadCursorW},
    },
};

const SAMPLE_INTERVAL: Duration = Duration::from_millis(16);
const KEYBOARD_SAMPLE_INTERVAL: Duration = Duration::from_millis(48);
const KEY_ACTIVITY_DEBOUNCE: Duration = Duration::from_millis(120);

#[derive(Serialize)]
struct TraceMetadata {
    #[serde(rename = "type")]
    line_type: &'static str,
    schema_version: u32,
    origin_x: i32,
    origin_y: i32,
    width: i32,
    height: i32,
    sample_interval_ms: u64,
}

#[derive(Serialize)]
struct TraceEvent {
    #[serde(rename = "type")]
    line_type: &'static str,
    time_ms: u64,
    time: f64,
    x: i32,
    y: i32,
    normalized_x: f64,
    normalized_y: f64,
    speed_px_s: f64,
    click: bool,
    event: &'static str,
    cursor_role: &'static str,
}

#[derive(Serialize)]
struct TraceEnd {
    #[serde(rename = "type")]
    line_type: &'static str,
    time_ms: u64,
}

#[derive(Clone, Copy)]
pub struct CaptureBounds {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl CaptureBounds {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Result<Self, String> {
        if width <= 0 || height <= 0 {
            return Err("选择的显示器尺寸无效".to_string());
        }
        Ok(Self { x, y, width, height })
    }

    fn normalize(self, point: POINT) -> (f64, f64) {
        let x = f64::from(point.x - self.x) / f64::from(self.width);
        let y = f64::from(point.y - self.y) / f64::from(self.height);
        (x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))
    }
}

pub struct MouseTrackerHandle {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<Result<(), String>>>,
}

impl MouseTrackerHandle {
    pub fn start(path: &Path, origin: Instant, bounds: CaptureBounds) -> Result<Self, String> {
        let file = File::create(path).map_err(|error| format!("无法创建鼠标轨迹文件：{error}"))?;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);

        let worker = thread::Builder::new()
            .name("focusflow-mouse-tracker".to_string())
            .spawn(move || track_mouse(file, bounds, origin, &worker_stop))
            .map_err(|error| format!("无法启动鼠标追踪：{error}"))?;

        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }

    pub fn stop(mut self) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        let worker = self
            .worker
            .take()
            .ok_or_else(|| "鼠标追踪已停止".to_string())?;
        worker
            .join()
            .map_err(|_| "鼠标追踪线程异常退出".to_string())?
    }
}

fn track_mouse(
    file: File,
    bounds: CaptureBounds,
    origin: Instant,
    stop: &AtomicBool,
) -> Result<(), String> {
    let mut writer = BufWriter::new(file);
    write_json_line(
        &mut writer,
        &TraceMetadata {
            line_type: "metadata",
            schema_version: 3,
            origin_x: bounds.x,
            origin_y: bounds.y,
            width: bounds.width,
            height: bounds.height,
            sample_interval_ms: SAMPLE_INTERVAL.as_millis() as u64,
        },
    )?;

    let mut previous_point = cursor_position()?;
    let mut previous_time = origin;
    let mut left_was_down = button_is_down(VK_LBUTTON);
    let mut right_was_down = button_is_down(VK_RBUTTON);
    let mut keyboard_state = current_keyboard_state();
    let mut last_keyboard_sample = origin;
    let mut last_key_activity: Option<Instant> = None;
    write_event(
        &mut writer,
        bounds,
        origin,
        previous_point,
        0.0,
        false,
        "move",
    )?;

    while !stop.load(Ordering::Acquire) {
        thread::sleep(SAMPLE_INTERVAL);
        let now = Instant::now();
        let point = cursor_position()?;
        let elapsed = now.duration_since(previous_time).as_secs_f64();
        let dx = f64::from(point.x - previous_point.x);
        let dy = f64::from(point.y - previous_point.y);
        let speed = if elapsed > 0.0 {
            dx.hypot(dy) / elapsed
        } else {
            0.0
        };

        let left_down = button_is_down(VK_LBUTTON);
        let right_down = button_is_down(VK_RBUTTON);
        let left_pressed = left_down && !left_was_down;
        let left_released = !left_down && left_was_down;
        let right_pressed = right_down && !right_was_down;
        let right_released = !right_down && right_was_down;
        let moved = point != previous_point;

        if moved {
            write_event(&mut writer, bounds, origin, point, speed, false, "move")?;
        }
        if left_pressed {
            write_event(&mut writer, bounds, origin, point, speed, false, "left_down")?;
            write_event(
                &mut writer,
                bounds,
                origin,
                point,
                speed,
                true,
                "left_click",
            )?;
        }
        if left_released {
            write_event(&mut writer, bounds, origin, point, speed, false, "left_up")?;
        }
        if right_pressed {
            write_event(&mut writer, bounds, origin, point, speed, false, "right_down")?;
            write_event(
                &mut writer,
                bounds,
                origin,
                point,
                speed,
                true,
                "right_click",
            )?;
        }
        if right_released {
            write_event(&mut writer, bounds, origin, point, speed, false, "right_up")?;
        }
        if now.duration_since(last_keyboard_sample) >= KEYBOARD_SAMPLE_INTERVAL {
            let (next_keyboard_state, key_pressed) = keyboard_activity(&keyboard_state);
            keyboard_state = next_keyboard_state;
            last_keyboard_sample = now;
            let debounce_elapsed = last_key_activity
                .is_none_or(|last| now.duration_since(last) >= KEY_ACTIVITY_DEBOUNCE);
            if key_pressed && debounce_elapsed {
                write_event(
                    &mut writer,
                    bounds,
                    origin,
                    point,
                    speed,
                    false,
                    "key_activity",
                )?;
                last_key_activity = Some(now);
            }
        }

        previous_point = point;
        previous_time = now;
        left_was_down = left_down;
        right_was_down = right_down;
    }
    let elapsed = origin.elapsed();
    write_json_line(
        &mut writer,
        &TraceEnd {
            line_type: "end",
            time_ms: elapsed.as_millis().try_into().unwrap_or(u64::MAX),
        },
    )?;
    writer
        .flush()
        .map_err(|error| format!("无法保存鼠标轨迹：{error}"))
}

fn cursor_position() -> Result<POINT, String> {
    let mut point = POINT::default();
    // SAFETY: point is a valid mutable POINT for the duration of the call.
    unsafe { GetCursorPos(&mut point) }.map_err(|error| format!("无法读取鼠标位置：{error}"))?;
    Ok(point)
}

fn button_is_down(button: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    // SAFETY: GetAsyncKeyState accepts any virtual-key code and has no pointer parameters.
    unsafe { GetAsyncKeyState(i32::from(button.0)) < 0 }
}

fn current_keyboard_state() -> [bool; 256] {
    let mut state = [false; 256];
    for key in 8_u16..=254 {
        state[usize::from(key)] = key_is_down(key);
    }
    state
}

fn keyboard_activity(previous: &[bool; 256]) -> ([bool; 256], bool) {
    let current = current_keyboard_state();
    let pressed = (8_usize..=254).any(|key| current[key] && !previous[key]);
    (current, pressed)
}

fn key_is_down(key: u16) -> bool {
    // SAFETY: GetAsyncKeyState accepts any virtual-key code and has no pointer parameters.
    unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
}

fn write_event(
    writer: &mut BufWriter<File>,
    bounds: CaptureBounds,
    origin: Instant,
    point: POINT,
    speed_px_s: f64,
    click: bool,
    event: &'static str,
) -> Result<(), String> {
    let elapsed = origin.elapsed();
    let (normalized_x, normalized_y) = bounds.normalize(point);
    write_json_line(
        writer,
        &TraceEvent {
            line_type: "mouse",
            time_ms: elapsed.as_millis().try_into().unwrap_or(u64::MAX),
            time: elapsed.as_secs_f64(),
            x: point.x,
            y: point.y,
            normalized_x,
            normalized_y,
            speed_px_s,
            click,
            event,
            cursor_role: current_cursor_role(),
        },
    )
}

fn current_cursor_role() -> &'static str {
    static HANDLES: OnceLock<Vec<(&'static str, usize)>> = OnceLock::new();
    let handles = HANDLES.get_or_init(|| {
        crate::cursor_render::CURSOR_ROLES
            .iter()
            .filter_map(|(role, id)| {
                unsafe {
                    LoadCursorW(
                        None,
                        windows::core::PCWSTR(usize::from(*id) as *const u16),
                    )
                }
                .ok()
                .map(|cursor| (*role, cursor.0 as usize))
            })
            .collect()
    });
    let mut info = CURSORINFO {
        cbSize: std::mem::size_of::<CURSORINFO>() as u32,
        ..Default::default()
    };
    if unsafe { GetCursorInfo(&mut info) }.is_err() || info.flags != CURSOR_SHOWING {
        return "normal";
    }
    handles
        .iter()
        .find_map(|(role, handle)| (*handle == info.hCursor.0 as usize).then_some(*role))
        .unwrap_or("normal")
}

fn write_json_line<T: Serialize>(writer: &mut BufWriter<File>, value: &T) -> Result<(), String> {
    serde_json::to_writer(&mut *writer, value)
        .map_err(|error| format!("无法编码鼠标轨迹：{error}"))?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("无法写入鼠标轨迹：{error}"))
}
