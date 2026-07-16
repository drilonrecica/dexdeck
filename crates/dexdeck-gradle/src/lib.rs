//! Gradle discovery and bridge integration.

mod discovery;

pub use discovery::{DiscoveryError, ProjectDiscovery, discover_project};
