//! Android SDK, device, emulator, and application services.

mod application;
mod device;
mod emulator;
mod logcat;
mod sdk;

pub use application::{ApplicationError, ApplicationService, InstallOptions};
pub use device::{AdbClient, AdbError, DeviceSelector, DeviceTracker, parse_device_list};
pub use emulator::{EmulatorError, EmulatorLaunch, EmulatorService};
pub use logcat::{LogcatParser, ParserStats};
pub use sdk::{Doctor, SdkError, SdkResolution, SdkResolver};
