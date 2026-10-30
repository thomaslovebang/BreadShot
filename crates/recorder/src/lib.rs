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
