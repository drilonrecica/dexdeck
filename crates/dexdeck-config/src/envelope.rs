use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionedEnvelope<T> {
    pub schema_version: u32,
    pub data: T,
}

impl<T> VersionedEnvelope<T> {
    #[must_use]
    pub const fn new(schema_version: u32, data: T) -> Self {
        Self {
            schema_version,
            data,
        }
    }
}
