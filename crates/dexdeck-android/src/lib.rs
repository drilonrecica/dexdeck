//! Android SDK, device, emulator, and application services.

mod device;
mod sdk;

pub use device::{AdbClient, AdbError, DeviceSelector, DeviceTracker, parse_device_list};
pub use sdk::{Doctor, SdkError, SdkResolution, SdkResolver};
