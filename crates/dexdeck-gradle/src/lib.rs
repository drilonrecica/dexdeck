//! Gradle discovery and bridge integration.

mod agp8;
mod agp9;
mod bridge;
mod discovery;
mod embedded;

pub use agp8::Agp8ModelBuilder;
pub use agp9::Agp9ModelBuilder;
pub use bridge::{
    AdapterKind, BRIDGE_JAVA_VERSION, BridgeError, BridgeInvocation, MODEL_TASK, OUTPUT_PROPERTY,
    select_adapter,
};
pub use discovery::{DiscoveryError, ProjectDiscovery, discover_project};
pub use embedded::{
    EmbeddedBridgeError, ExtractedBridge, extract_bridge, parse_complete_output, select_gradle,
};
