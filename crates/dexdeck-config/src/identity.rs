use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::StorageError;

pub const PROJECT_NAMESPACE_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectIdentity {
    canonical_root: PathBuf,
    hash: String,
}

impl ProjectIdentity {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Self::from_path_with_namespace(path, PROJECT_NAMESPACE_VERSION)
    }

    pub fn from_path_with_namespace(
        path: impl AsRef<Path>,
        namespace_version: u32,
    ) -> Result<Self, StorageError> {
        let input = path.as_ref();
        let canonical_root =
            std::fs::canonicalize(input).map_err(|source| StorageError::io(input, source))?;
        let mut digest = Sha256::new();
        digest.update(b"dexdeck-project\0");
        digest.update(namespace_version.to_le_bytes());
        update_path_digest(&mut digest, &canonical_root);
        let hash = to_lower_hex(&digest.finalize());

        Ok(Self {
            canonical_root,
            hash,
        })
    }

    #[must_use]
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    #[must_use]
    pub fn hash(&self) -> &str {
        &self.hash
    }
}

#[cfg(unix)]
fn update_path_digest(digest: &mut Sha256, path: &Path) {
    use std::os::unix::ffi::OsStrExt;

    digest.update(path.as_os_str().as_bytes());
}

#[cfg(windows)]
fn update_path_digest(digest: &mut Sha256, path: &Path) {
    use std::os::windows::ffi::OsStrExt;

    for unit in path.as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
}

fn to_lower_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn identity_is_stable_and_namespaced() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempdir()?;
        let first = ProjectIdentity::from_path(directory.path())?;
        let second = ProjectIdentity::from_path(directory.path().join("."))?;
        let future = ProjectIdentity::from_path_with_namespace(directory.path(), 2)?;

        assert_eq!(first, second);
        assert_ne!(first.hash(), future.hash());
        assert_eq!(first.hash().len(), 64);
        assert!(
            !first
                .hash()
                .contains(&directory.path().to_string_lossy()[..])
        );
        Ok(())
    }
}
