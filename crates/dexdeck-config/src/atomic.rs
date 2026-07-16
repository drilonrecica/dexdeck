use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Serialize, de::DeserializeOwned};
use tempfile::NamedTempFile;

use crate::{StorageError, VersionedEnvelope};

pub const MAX_LOCAL_STATE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub enum RecoveredFile<T> {
    Missing,
    Loaded(T),
    Corrupt {
        quarantined_path: Option<PathBuf>,
        message: String,
    },
}

pub fn write_json_atomic<T: Serialize>(
    path: impl AsRef<Path>,
    value: &VersionedEnvelope<T>,
) -> Result<(), StorageError> {
    let path = path.as_ref();
    reject_symlink(path)?;
    let parent = usable_parent(path).ok_or_else(|| {
        StorageError::io(
            path,
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"),
        )
    })?;
    ensure_private_directory(parent)?;

    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| StorageError::io(parent, source))?;
    set_private_file_permissions(temporary.as_file(), temporary.path())?;
    {
        let mut writer = BufWriter::new(temporary.as_file_mut());
        serde_json::to_writer(&mut writer, value).map_err(|source| StorageError::Serialize {
            path: path.to_path_buf(),
            source,
        })?;
        writer
            .write_all(b"\n")
            .map_err(|source| StorageError::io(path, source))?;
        writer
            .flush()
            .map_err(|source| StorageError::io(path, source))?;
    }
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| StorageError::io(path, source))?;
    temporary
        .persist(path)
        .map_err(|error| StorageError::io(path, error.error))?;
    sync_parent_directory(parent)?;
    Ok(())
}

pub fn write_text_atomic(path: impl AsRef<Path>, text: &str) -> Result<(), StorageError> {
    let path = path.as_ref();
    reject_unsafe_components(path)?;
    let parent = usable_parent(path).ok_or_else(|| {
        StorageError::io(
            path,
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "path has no parent"),
        )
    })?;
    fs::create_dir_all(parent).map_err(|source| StorageError::io(parent, source))?;

    let mut temporary =
        NamedTempFile::new_in(parent).map_err(|source| StorageError::io(parent, source))?;
    temporary
        .write_all(text.as_bytes())
        .and_then(|()| temporary.flush())
        .map_err(|source| StorageError::io(path, source))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|source| StorageError::io(path, source))?;
    temporary
        .persist(path)
        .map_err(|error| StorageError::io(path, error.error))?;
    sync_parent_directory(parent)?;
    Ok(())
}

pub fn load_json<T: DeserializeOwned>(
    path: impl AsRef<Path>,
    expected_schema_version: u32,
) -> Result<Option<T>, StorageError> {
    let path = path.as_ref();
    reject_symlink(path)?;
    let file = match File::open(path) {
        Ok(file) => file,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(StorageError::io(path, source)),
    };
    let size = file
        .metadata()
        .map_err(|source| StorageError::io(path, source))?
        .len();
    if size > MAX_LOCAL_STATE_BYTES {
        return Err(StorageError::FileTooLarge {
            path: path.to_path_buf(),
            size,
            maximum: MAX_LOCAL_STATE_BYTES,
        });
    }

    let envelope = serde_json::from_reader::<_, VersionedEnvelope<T>>(BufReader::new(file))
        .map_err(|source| StorageError::CorruptData {
            path: path.to_path_buf(),
            message: source.to_string(),
        })?;
    if envelope.schema_version != expected_schema_version {
        return Err(StorageError::UnsupportedSchema {
            path: path.to_path_buf(),
            expected: expected_schema_version,
            found: envelope.schema_version,
        });
    }
    Ok(Some(envelope.data))
}

pub fn load_json_recovering<T: DeserializeOwned>(
    path: impl AsRef<Path>,
    expected_schema_version: u32,
) -> Result<RecoveredFile<T>, StorageError> {
    let path = path.as_ref();
    match load_json(path, expected_schema_version) {
        Ok(Some(value)) => Ok(RecoveredFile::Loaded(value)),
        Ok(None) => Ok(RecoveredFile::Missing),
        Err(
            error @ (StorageError::CorruptData { .. } | StorageError::UnsupportedSchema { .. }),
        ) => {
            let quarantined_path = quarantine(path).ok();
            Ok(RecoveredFile::Corrupt {
                quarantined_path,
                message: error.to_string(),
            })
        }
        Err(error) => Err(error),
    }
}

fn reject_symlink(path: &Path) -> Result<(), StorageError> {
    reject_unsafe_components(path)
}

fn reject_unsafe_components(path: &Path) -> Result<(), StorageError> {
    for component in path.ancestors() {
        if component.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(component) {
            Ok(metadata)
                if is_link_or_reparse_point(&metadata)
                    && !is_macos_system_directory_alias(component) =>
            {
                return Err(StorageError::UnsafePath {
                    path: component.to_path_buf(),
                });
            }
            Ok(_) => {}
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(StorageError::io(component, source)),
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn is_macos_system_directory_alias(path: &Path) -> bool {
    let expected = match path.to_str() {
        Some("/etc") => "private/etc",
        Some("/tmp") => "private/tmp",
        Some("/var") => "private/var",
        _ => return false,
    };
    fs::read_link(path).is_ok_and(|target| {
        target == Path::new(expected) || target == Path::new("/").join(expected)
    })
}

#[cfg(not(target_os = "macos"))]
fn is_macos_system_directory_alias(_path: &Path) -> bool {
    false
}

fn usable_parent(path: &Path) -> Option<&Path> {
    path.parent().map(|parent| {
        if parent.as_os_str().is_empty() {
            Path::new(".")
        } else {
            parent
        }
    })
}

#[cfg(not(windows))]
fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_symlink()
}

#[cfg(windows)]
fn is_link_or_reparse_point(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    metadata.file_type().is_symlink()
        || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

fn ensure_private_directory(path: &Path) -> Result<(), StorageError> {
    fs::create_dir_all(path).map_err(|source| StorageError::io(path, source))?;
    set_private_directory_permissions(path)?;
    Ok(())
}

fn quarantine(path: &Path) -> Result<PathBuf, StorageError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("state");
    let quarantined = path.with_file_name(format!("{file_name}.corrupt-{timestamp}"));
    fs::rename(path, &quarantined).map_err(|source| StorageError::io(path, source))?;
    Ok(quarantined)
}

#[cfg(unix)]
fn set_private_directory_permissions(path: &Path) -> Result<(), StorageError> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|source| StorageError::io(path, source))
}

#[cfg(windows)]
fn set_private_directory_permissions(path: &Path) -> Result<(), StorageError> {
    restrict_windows_acl(path, true)
}

#[cfg(all(not(unix), not(windows)))]
fn set_private_directory_permissions(_path: &Path) -> Result<(), StorageError> {
    Ok(())
}

#[cfg(unix)]
fn set_private_file_permissions(file: &File, path: &Path) -> Result<(), StorageError> {
    use std::os::unix::fs::PermissionsExt;

    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|source| StorageError::io(path, source))
}

#[cfg(windows)]
fn set_private_file_permissions(_file: &File, path: &Path) -> Result<(), StorageError> {
    restrict_windows_acl(path, false)
}

#[cfg(all(not(unix), not(windows)))]
fn set_private_file_permissions(_file: &File, _path: &Path) -> Result<(), StorageError> {
    Ok(())
}

#[cfg(windows)]
fn restrict_windows_acl(path: &Path, directory: bool) -> Result<(), StorageError> {
    let user = std::env::var_os("USERNAME").ok_or_else(|| {
        StorageError::io(
            path,
            std::io::Error::other("USERNAME is unavailable for ACL setup"),
        )
    })?;
    let mut grant = user;
    grant.push(if directory { ":(OI)(CI)F" } else { ":F" });
    let status = std::process::Command::new("icacls.exe")
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .arg(grant)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|source| StorageError::io(path, source))?;
    if !status.success() {
        return Err(StorageError::io(
            path,
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "icacls rejected private ACL",
            ),
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), StorageError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| StorageError::io(path, source))
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), StorageError> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    use super::*;

    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
    struct State {
        selection: String,
    }

    #[test]
    fn atomically_replaces_and_loads_state() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("nested").join("state.json");

        write_json_atomic(
            &path,
            &VersionedEnvelope::new(
                1,
                State {
                    selection: "first".into(),
                },
            ),
        )?;
        write_json_atomic(
            &path,
            &VersionedEnvelope::new(
                1,
                State {
                    selection: "second".into(),
                },
            ),
        )?;

        assert_eq!(
            load_json::<State>(&path, 1)?,
            Some(State {
                selection: "second".into(),
            })
        );
        Ok(())
    }

    #[test]
    fn corrupt_state_is_quarantined() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("state.json");
        fs::write(&path, b"not-json")?;

        match load_json_recovering::<State>(&path, 1)? {
            RecoveredFile::Corrupt {
                quarantined_path: Some(quarantined),
                ..
            } => {
                assert!(quarantined.exists());
                assert!(!path.exists());
            }
            other => panic!("expected quarantined corrupt state, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn schema_mismatch_is_recoverable() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("state.json");
        write_json_atomic(
            &path,
            &VersionedEnvelope::new(
                2,
                State {
                    selection: "future".into(),
                },
            ),
        )?;

        assert!(matches!(
            load_json_recovering::<State>(&path, 1)?,
            RecoveredFile::Corrupt { .. }
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_link_targets() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let directory = tempdir()?;
        let target = directory.path().join("target.json");
        let link = directory.path().join("link.json");
        fs::write(&target, b"{}")?;
        symlink(&target, &link)?;

        assert!(matches!(
            load_json::<State>(&link, 1),
            Err(StorageError::UnsafePath { .. })
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_in_parent_components() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::symlink;

        let directory = tempdir()?;
        let target = directory.path().join("target");
        fs::create_dir(&target)?;
        let link = directory.path().join("link");
        symlink(&target, &link)?;
        let path = link.join("state.json");

        assert!(matches!(
            write_json_atomic(
                &path,
                &VersionedEnvelope::new(
                    1,
                    State {
                        selection: "unsafe".into()
                    }
                )
            ),
            Err(StorageError::UnsafePath { .. })
        ));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn writes_private_file_and_directory_modes() -> Result<(), Box<dyn std::error::Error>> {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempdir()?;
        let private_directory = directory.path().join("private");
        let path = private_directory.join("state.json");
        write_json_atomic(
            &path,
            &VersionedEnvelope::new(
                1,
                State {
                    selection: "safe".into(),
                },
            ),
        )?;

        assert_eq!(
            fs::metadata(&private_directory)?.permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o600);
        Ok(())
    }
}
