use std::path::Path;

use dexdeck_protocol::{LogFilterSpec, ProjectModel};
use serde::{Deserialize, Serialize};

use crate::{
    RecoveredFile, StorageError, VersionedEnvelope, load_json_recovering, write_json_atomic,
};

pub const SESSION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_filter: Option<LogFilterSpec>,
    #[serde(default)]
    pub reduced_motion: bool,
}

impl SessionState {
    pub fn retain_valid(&mut self, model: Option<&ProjectModel>, device_serials: &[String]) {
        let module = self.module.as_deref();
        let selected_module = model.and_then(|model| {
            model
                .modules
                .iter()
                .find(|candidate| Some(candidate.path.as_str()) == module)
        });
        if selected_module.is_none() {
            self.module = None;
            self.variant = None;
        } else if !selected_module.is_some_and(|module| {
            module
                .variants
                .iter()
                .any(|variant| Some(variant.name.as_str()) == self.variant.as_deref())
        }) {
            self.variant = None;
        }
        if !device_serials
            .iter()
            .any(|serial| Some(serial.as_str()) == self.device.as_deref())
        {
            self.device = None;
        }
    }
}

pub fn load_session(path: &Path) -> Result<RecoveredFile<SessionState>, StorageError> {
    load_json_recovering(path, SESSION_SCHEMA_VERSION)
}

pub fn save_session(path: &Path, session: &SessionState) -> Result<(), StorageError> {
    write_json_atomic(
        path,
        &VersionedEnvelope::new(SESSION_SCHEMA_VERSION, session),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_and_device_clear_stale_selections() {
        let mut session = SessionState {
            module: Some(":app".into()),
            variant: Some("debug".into()),
            device: Some("gone".into()),
            ..SessionState::default()
        };
        session.retain_valid(None, &[]);
        assert_eq!(session.module, None);
        assert_eq!(session.variant, None);
        assert_eq!(session.device, None);
    }
}
