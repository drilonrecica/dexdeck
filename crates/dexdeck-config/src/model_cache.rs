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
    #[serde(default)]
    pub bridge_version: String,
    #[serde(default)]
    pub model_hash: String,
    pub inputs: Vec<ModelInput>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelCacheBundle {
    pub model: ProjectModel,
    pub fingerprint: ModelFingerprint,
}

pub fn discover_model_inputs(root: &Path) -> io::Result<Vec<PathBuf>> {
    let mut inputs = Vec::new();
    collect_inputs(root, root, 10, false, &mut inputs)?;
    inputs.sort();
    inputs.dedup();
    Ok(inputs)
}

fn collect_inputs(
    root: &Path,
    directory: &Path,
    depth: usize,
    convention: bool,
    inputs: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if depth == 0 {
        return Ok(());
    }
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if metadata.is_dir() {
            if matches!(
                name.as_ref(),
                ".git" | ".gradle" | "build" | "out" | "target"
            ) {
                continue;
            }
            let is_convention = convention || matches!(name.as_ref(), "buildSrc" | "build-logic");
            // Ordinary Android/Kotlin/Java source trees do not affect the Gradle model.
            if name == "src" && !is_convention {
                continue;
            }
            collect_inputs(root, &path, depth - 1, is_convention, inputs)?;
        } else if is_known_model_file(root, &path, convention) {
            inputs.push(path);
        }
    }
    Ok(())
}

fn is_known_model_file(root: &Path, path: &Path, convention: bool) -> bool {
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    let relative = path.strip_prefix(root).unwrap_or(path);
    matches!(
        name,
        "settings.gradle"
            | "settings.gradle.kts"
            | "build.gradle"
            | "build.gradle.kts"
            | "gradle.properties"
            | "gradle-wrapper.properties"
            | "libs.versions.toml"
            | "config.toml"
    ) && (name != "config.toml" || relative == Path::new(".dexdeck/config.toml"))
        || convention
            && matches!(
                path.extension().and_then(|value| value.to_str()),
                Some("kt" | "kts" | "java" | "groovy" | "toml")
            )
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
        let sha256 = match old {
            Some(input) => input.sha256.clone(),
            None => format!("{:x}", Sha256::digest(fs::read(path)?)),
        };
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
        bridge_version: previous.map_or_else(String::new, |value| value.bridge_version.clone()),
        model_hash: previous.map_or_else(String::new, |value| value.model_hash.clone()),
        inputs,
    })
}

pub fn fingerprint_for_model(
    paths: &[PathBuf],
    previous: Option<&ModelFingerprint>,
    bridge_version: impl Into<String>,
    model_hash: impl Into<String>,
) -> io::Result<ModelFingerprint> {
    let mut value = fingerprint(paths, previous)?;
    value.bridge_version = bridge_version.into();
    value.model_hash = model_hash.into();
    Ok(value)
}

pub fn save_fingerprint(path: &Path, value: &ModelFingerprint) -> Result<(), crate::StorageError> {
    write_json_atomic(path, &VersionedEnvelope::new(CACHE_SCHEMA_VERSION, value))
}

pub fn load_fingerprint(path: &Path) -> Result<Option<ModelFingerprint>, crate::StorageError> {
    Ok(match load_json_recovering(path, CACHE_SCHEMA_VERSION)? {
        RecoveredFile::Loaded(value) => Some(value),
        RecoveredFile::Missing | RecoveredFile::Corrupt { .. } => None,
    })
}

pub fn save_model_bundle(path: &Path, value: &ModelCacheBundle) -> Result<(), crate::StorageError> {
    write_json_atomic(path, &VersionedEnvelope::new(CACHE_SCHEMA_VERSION, value))
}

pub fn load_model_bundle(
    path: &Path,
) -> Result<RecoveredFile<ModelCacheBundle>, crate::StorageError> {
    load_json_recovering(path, CACHE_SCHEMA_VERSION)
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

    #[test]
    fn model_and_fingerprint_commit_as_one_bundle() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("model-bundle.json");
        let value = ModelCacheBundle {
            model: ProjectModel::empty(directory.path().to_path_buf()),
            fingerprint: ModelFingerprint {
                schema_version: CACHE_SCHEMA_VERSION,
                bridge_version: "bridge".into(),
                model_hash: "hash".into(),
                inputs: Vec::new(),
            },
        };
        save_model_bundle(&path, &value)?;
        assert!(
            matches!(load_model_bundle(&path)?, RecoveredFile::Loaded(loaded) if loaded == value)
        );
        Ok(())
    }
}
