mod epics;
mod error;
mod host;
mod wire;

pub use dcode_client::HostEndpoint;
pub use epics::list_epics;
pub use error::{Error, Result};
pub use host::host_status;
pub use wire::{EpicSummary, HostStatus};
