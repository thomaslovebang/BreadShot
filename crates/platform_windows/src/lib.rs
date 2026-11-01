#![doc = "Windows-only screen capture and input adapters."]
#![allow(unsafe_code)]

#[cfg(not(target_os = "windows"))]
compile_error!("focusflow-platform-windows only supports Windows");

pub const PLATFORM_NAME: &str = "windows";

mod mouse_tracker;
pub mod cursor_render;

pub use mouse_tracker::{CaptureBounds, MouseTrackerHandle};
