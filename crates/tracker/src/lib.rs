#![doc = "Mouse event acquisition and normalization boundary."]

use focusflow_domain::MouseEvent;

pub trait MouseEventSink {
    type Error;

    fn push(&mut self, event: MouseEvent) -> Result<(), Self::Error>;
}
