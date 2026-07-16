//! Gradle discovery and bridge integration.

mod bridge;
mod discovery;

pub use bridge::{
    AdapterKind, BRIDGE_JAVA_VERSION, BridgeError, BridgeInvocation, MODEL_TASK, OUTPUT_PROPERTY,
    select_adapter,
};
pub use discovery::{DiscoveryError, ProjectDiscovery, discover_project};
