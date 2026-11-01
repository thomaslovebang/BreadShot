use std::{
    collections::VecDeque,
    fs::File,
    io::Write,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
    time::Instant,
};
use wasapi::{
    Device, DeviceEnumerator, Direction, SampleType, StreamMode, WaveFormat, initialize_mta,
};

const SAMPLE_RATE: u32 = 44_100;
const CHANNELS: usize = 2;
const BITS_PER_SAMPLE: usize = 32;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub struct AudioCaptureHandle {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<Result<(), String>>>,
    pub path: PathBuf,
    pub started_at: Instant,
}

impl AudioCaptureHandle {
    pub fn stop(mut self) -> Result<PathBuf, String> {
        self.stop.store(true, Ordering::Release);
        let result = self
            .thread
            .take()
            .ok_or_else(|| "音频采集线程不存在".to_string())?
            .join()
            .map_err(|_| "音频采集线程异常退出".to_string())?;
        result?;
        Ok(self.path)
    }
}

pub fn list_microphones() -> Result<Vec<AudioDeviceInfo>, String> {
    initialize_mta().ok().map_err(|error| format!("无法初始化 Windows 音频：{error}"))?;
    let enumerator = DeviceEnumerator::new().map_err(audio_error)?;
    let default_name = enumerator
        .get_default_device(&Direction::Capture)
        .ok()
        .and_then(|device| device.get_friendlyname().ok());
    let collection = enumerator
        .get_device_collection(&Direction::Capture)
        .map_err(audio_error)?;
    let mut devices = Vec::new();
    for device in &collection {
        let device = device.map_err(audio_error)?;
        let name = device.get_friendlyname().map_err(audio_error)?;
        devices.push(AudioDeviceInfo {
            id: name.clone(),
            is_default: default_name.as_deref() == Some(name.as_str()),
            name,
        });
    }
    devices.sort_by_key(|device| !device.is_default);
    Ok(devices)
}

pub fn system_audio_available() -> Result<bool, String> {
    initialize_mta().ok().map_err(|error| format!("无法初始化 Windows 音频：{error}"))?;
    let enumerator = DeviceEnumerator::new().map_err(audio_error)?;
    Ok(enumerator.get_default_device(&Direction::Render).is_ok())
}

pub fn start_system_audio(path: PathBuf) -> Result<AudioCaptureHandle, String> {
    start_capture(path, CaptureSource::System)
}

pub fn start_microphone(path: PathBuf, device_name: Option<String>) -> Result<AudioCaptureHandle, String> {
    start_capture(path, CaptureSource::Microphone(device_name))
}

enum CaptureSource {
    System,
    Microphone(Option<String>),
}

fn start_capture(path: PathBuf, source: CaptureSource) -> Result<AudioCaptureHandle, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let thread_path = path.clone();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let thread = thread::Builder::new()
        .name("focusflow-audio-capture".to_string())
        .spawn(move || capture_loop(thread_path, source, thread_stop, ready_tx))
        .map_err(|error| format!("无法启动音频采集线程：{error}"))?;

    match ready_rx.recv_timeout(Duration::from_secs(6)) {
        Ok(Ok(())) => Ok(AudioCaptureHandle {
            stop,
            thread: Some(thread),
            path,
            started_at: Instant::now(),
        }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(error)
        }
        Err(_) => {
            stop.store(true, Ordering::Release);
            let _ = thread.join();
            Err("启动音频设备超时".to_string())
        }
    }
}

fn capture_loop(
    path: PathBuf,
    source: CaptureSource,
    stop: Arc<AtomicBool>,
    ready: mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let result = (|| {
        initialize_mta().ok().map_err(|error| format!("无法初始化 Windows 音频线程：{error}"))?;
        let enumerator = DeviceEnumerator::new().map_err(audio_error)?;
        let (device, endpoint_direction) = match source {
            CaptureSource::System => (
                enumerator
                    .get_default_device(&Direction::Render)
                    .map_err(audio_error)?,
                Direction::Render,
            ),
            CaptureSource::Microphone(name) => (
                microphone_device(&enumerator, name.as_deref())?,
                Direction::Capture,
            ),
        };
        run_device_capture(device, endpoint_direction, &path, &stop, &ready)
    })();
    if let Err(error) = &result {
        let _ = ready.try_send(Err(error.clone()));
    }
    result
}

fn microphone_device(enumerator: &DeviceEnumerator, name: Option<&str>) -> Result<Device, String> {
    if let Some(name) = name {
        let collection = enumerator
            .get_device_collection(&Direction::Capture)
            .map_err(audio_error)?;
        for device in &collection {
            let device = device.map_err(audio_error)?;
            if device.get_friendlyname().map_err(audio_error)? == name {
                return Ok(device);
            }
        }
        return Err(format!("找不到麦克风：{name}"));
    }
    enumerator
        .get_default_device(&Direction::Capture)
        .map_err(audio_error)
}

fn run_device_capture(
    device: Device,
    endpoint_direction: Direction,
    path: &PathBuf,
    stop: &AtomicBool,
    ready: &mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let mut audio_client = device.get_iaudioclient().map_err(audio_error)?;
    let format = WaveFormat::new(
        BITS_PER_SAMPLE,
        BITS_PER_SAMPLE,
        &SampleType::Float,
        SAMPLE_RATE as usize,
        CHANNELS,
        None,
    );
    let block_align = format.get_blockalign() as usize;
    let (_, minimum_period) = audio_client.get_device_period().map_err(audio_error)?;
    let mode = StreamMode::EventsShared {
        autoconvert: true,
        buffer_duration_hns: minimum_period,
    };
    audio_client
        .initialize_client(&format, &Direction::Capture, &mode)
        .map_err(audio_error)?;
    let event = audio_client.set_get_eventhandle().map_err(audio_error)?;
    let capture = audio_client.get_audiocaptureclient().map_err(audio_error)?;
    let mut queue = VecDeque::new();
    let mut output = File::create(path)
        .map_err(|error| format!("无法创建音频缓存 {}：{error}", path.display()))?;
    audio_client.start_stream().map_err(audio_error)?;
    ready
        .send(Ok(()))
        .map_err(|_| "无法确认音频设备已启动".to_string())?;

    while !stop.load(Ordering::Acquire) {
        capture
            .read_from_device_to_deque(&mut queue)
            .map_err(audio_error)?;
        write_queue(&mut output, &mut queue)?;
        let _ = event.wait_for_event(100);
    }
    capture
        .read_from_device_to_deque(&mut queue)
        .map_err(audio_error)?;
    write_queue(&mut output, &mut queue)?;
    audio_client.stop_stream().map_err(audio_error)?;
    output.flush().map_err(|error| format!("无法保存音频缓存：{error}"))?;
    if output.metadata().map_err(|error| error.to_string())?.len() < block_align as u64 {
        let silence = vec![0_u8; SAMPLE_RATE as usize * block_align / 10];
        output
            .write_all(&silence)
            .map_err(|error| format!("无法补齐静音音频：{error}"))?;
    }
    let _ = endpoint_direction;
    Ok(())
}

fn write_queue(file: &mut File, queue: &mut VecDeque<u8>) -> Result<(), String> {
    let (first, second) = queue.as_slices();
    file.write_all(first).map_err(|error| format!("无法写入音频：{error}"))?;
    file.write_all(second).map_err(|error| format!("无法写入音频：{error}"))?;
    queue.clear();
    Ok(())
}

fn audio_error(error: impl std::fmt::Display) -> String {
    format!("Windows 音频错误：{error}")
}
