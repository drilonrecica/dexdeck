use std::{collections::HashSet, path::Path};

use dexdeck_protocol::{LOG_FILTERS_SCHEMA_VERSION, SavedLogFilterPreset};

use crate::{
    RecoveredFile, StorageError, VersionedEnvelope, load_json_recovering, write_json_atomic,
};

pub const SAVED_LOG_FILTER_LIMIT: usize = 100;
pub const SAVED_LOG_FILTER_NAME_LIMIT: usize = 64;

pub fn save_log_filters(
    path: &Path,
    presets: &[SavedLogFilterPreset],
) -> Result<(), LogFilterPresetError> {
    validate_presets(presets)?;
    write_json_atomic(
        path,
        &VersionedEnvelope::new(LOG_FILTERS_SCHEMA_VERSION, presets),
    )?;
    Ok(())
}

pub fn load_log_filters(
    path: &Path,
) -> Result<RecoveredFile<Vec<SavedLogFilterPreset>>, StorageError> {
    let loaded: RecoveredFile<Vec<SavedLogFilterPreset>> =
        load_json_recovering(path, LOG_FILTERS_SCHEMA_VERSION)?;
    Ok(match loaded {
        RecoveredFile::Loaded(presets) if validate_presets(&presets).is_ok() => {
            RecoveredFile::Loaded(presets)
        }
        RecoveredFile::Loaded(_) => RecoveredFile::Corrupt {
            quarantined_path: None,
            message: "saved Logcat filters failed validation".into(),
        },
        other => other,
    })
}

fn validate_presets(presets: &[SavedLogFilterPreset]) -> Result<(), LogFilterPresetError> {
    if presets.len() > SAVED_LOG_FILTER_LIMIT {
        return Err(LogFilterPresetError::TooMany(presets.len()));
    }
    let mut names = HashSet::with_capacity(presets.len());
    for preset in presets {
        let length = preset.name.chars().count();
        if preset.name.trim().is_empty() || length > SAVED_LOG_FILTER_NAME_LIMIT {
            return Err(LogFilterPresetError::InvalidName(preset.name.clone()));
        }
        if !names.insert(preset.name.clone()) {
            return Err(LogFilterPresetError::DuplicateName(preset.name.clone()));
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum LogFilterPresetError {
    #[error("at most {SAVED_LOG_FILTER_LIMIT} Logcat filter presets may be saved, got {0}")]
    TooMany(usize),
    #[error(
        "Logcat filter preset name must contain 1 to {SAVED_LOG_FILTER_NAME_LIMIT} characters: {0:?}"
    )]
    InvalidName(String),
    #[error("Logcat filter preset name is duplicated: {0:?}")]
    DuplicateName(String),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[cfg(test)]
mod tests {
    use dexdeck_protocol::LogFilterSpec;

    use super::*;

    #[test]
    fn saves_bounded_schema_v1_presets_and_recovers_corruption()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("filters.json");
        let presets = vec![SavedLogFilterPreset {
            name: "Errors".into(),
            filter: LogFilterSpec {
                errors: true,
                ..LogFilterSpec::default()
            },
        }];
        save_log_filters(&path, &presets)?;
        let RecoveredFile::Loaded(loaded) = load_log_filters(&path)? else {
            panic!("preset was not loaded")
        };
        assert_eq!(loaded, presets);
        std::fs::write(&path, "not-json")?;
        assert!(matches!(
            load_log_filters(&path)?,
            RecoveredFile::Corrupt { .. }
        ));
        Ok(())
    }

    #[test]
    fn rejects_empty_duplicate_and_excessive_names() {
        let preset = |name: &str| SavedLogFilterPreset {
            name: name.into(),
            filter: LogFilterSpec::default(),
        };
        assert!(validate_presets(&[preset("")]).is_err());
        assert!(validate_presets(&[preset("same"), preset("same")]).is_err());
        assert!(validate_presets(&[preset(&"x".repeat(65))]).is_err());
    }
}
