use crate::{RecoveredFile, VersionedEnvelope, load_json_recovering, write_json_atomic};
use dexdeck_protocol::{CACHE_SCHEMA_VERSION, ProjectModel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInput {
    pub path: PathBuf,
    pub size: u64,
    pub modified_ms: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelFingerprint {
    pub schema_version: u32,
    pub inputs: Vec<ModelInput>,
}

pub fn discover_model_inputs(root: &Path) -> io::Result<Vec<PathBuf>> {
    let candidates = [
        "settings.gradle",
        "settings.gradle.kts",
        "build.gradle",
        "build.gradle.kts",
        "gradle.properties",
        "gradle/libs.versions.toml",
        "gradle/wrapper/gradle-wrapper.properties",
        ".dexdeck/config.toml",
    ];
    Ok(candidates
        .iter()
        .map(|path| root.join(path))
        .filter(|path| path.is_file())
        .collect())
}

pub fn fingerprint(
    paths: &[PathBuf],
    previous: Option<&ModelFingerprint>,
) -> io::Result<ModelFingerprint> {
    let mut inputs = Vec::with_capacity(paths.len());
    for path in paths {
        let metadata = fs::metadata(path)?;
        let modified_ms = metadata
            .modified()
            .ok()
            .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |v| v.as_millis() as u64);
        let old = previous.and_then(|old| {
            old.inputs.iter().find(|input| {
                input.path == *path
                    && input.size == metadata.len()
                    && input.modified_ms == modified_ms
            })
        });
        let sha256 = old
            .map(|input| input.sha256.clone())
            .unwrap_or_else(|| format!("{:x}", Sha256::digest(fs::read(path).unwrap_or_default())));
        inputs.push(ModelInput {
            path: path.clone(),
            size: metadata.len(),
            modified_ms,
            sha256,
        });
    }
    inputs.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(ModelFingerprint {
        schema_version: CACHE_SCHEMA_VERSION,
        inputs,
    })
}

pub fn save_model(path: &Path, model: &ProjectModel) -> Result<(), crate::StorageError> {
    write_json_atomic(path, &VersionedEnvelope::new(CACHE_SCHEMA_VERSION, model))
}
pub fn load_model(path: &Path) -> Result<Option<ProjectModel>, crate::StorageError> {
    Ok(match load_json_recovering(path, CACHE_SCHEMA_VERSION)? {
        RecoveredFile::Loaded(model) => Some(model),
        RecoveredFile::Missing | RecoveredFile::Corrupt { .. } => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scans_only_known_inputs() {
        let temp = tempfile::tempdir().unwrap_or_else(|e| panic!("{e}"));
        fs::write(temp.path().join("settings.gradle"), "").unwrap_or_else(|e| panic!("{e}"));
        fs::write(temp.path().join("secret.txt"), "").unwrap_or_else(|e| panic!("{e}"));
        let files = discover_model_inputs(temp.path()).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(files.len(), 1);
    }
}
