use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::OnceLock,
};

use dexdeck_protocol::{BridgeEnvelope, BridgeStreamValidator};
use sha2::{Digest, Sha256};
use thiserror::Error;

const JAR: &[u8] = include_bytes!("../../../bridge/dexdeck-bridge.jar");
const INIT: &[u8] = include_bytes!("../../../bridge/dexdeck.init.gradle");

#[must_use]
pub fn embedded_bridge_hash() -> &'static str {
    static HASH: OnceLock<String> = OnceLock::new();
    HASH.get_or_init(|| {
        let mut digest = Sha256::new();
        digest.update(JAR);
        digest.update(INIT);
        format!("{:x}", digest.finalize())
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtractedBridge {
    pub directory: PathBuf,
    pub jar: PathBuf,
    pub init_script: PathBuf,
}

#[derive(Debug, Error)]
pub enum EmbeddedBridgeError {
    #[error("bridge I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("bridge protocol failed: {0}")]
    Protocol(#[from] dexdeck_protocol::BridgeProtocolError),
    #[error("project has no Gradle wrapper; system Gradle requires explicit approval")]
    WrapperRequired,
}

pub fn extract_bridge(cache: &Path) -> Result<ExtractedBridge, EmbeddedBridgeError> {
    let hash = embedded_bridge_hash();
    let directory = cache.join(hash);
    fs::create_dir_all(&directory)?;
    let jar = directory.join("dexdeck-bridge.jar");
    let init_script = directory.join("dexdeck.init.gradle");
    write_verified(&jar, JAR)?;
    write_verified(&init_script, INIT)?;
    Ok(ExtractedBridge {
        directory,
        jar,
        init_script,
    })
}

pub fn select_gradle(
    wrapper: Option<&Path>,
    system_approved: bool,
) -> Result<PathBuf, EmbeddedBridgeError> {
    wrapper
        .map(Path::to_path_buf)
        .or_else(|| system_approved.then(|| PathBuf::from("gradle")))
        .ok_or(EmbeddedBridgeError::WrapperRequired)
}

pub fn parse_complete_output(text: &str) -> Result<Vec<BridgeEnvelope>, EmbeddedBridgeError> {
    let mut validator = BridgeStreamValidator::default();
    let records = text
        .lines()
        .map(|line| validator.accept_json_line(line))
        .collect::<Result<Vec<_>, _>>()?;
    validator.finish()?;
    Ok(records)
}

fn write_verified(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if fs::read(path).is_ok_and(|existing| Sha256::digest(&existing) == Sha256::digest(bytes)) {
        return Ok(());
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    if Sha256::digest(fs::read(&temporary)?) != Sha256::digest(bytes) {
        return Err(io::Error::other("bridge hash mismatch"));
    }
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extracts_content_addressed_bridge() {
        let temp = tempfile::tempdir().unwrap_or_else(|error| panic!("tempdir: {error}"));
        let bridge = extract_bridge(temp.path()).unwrap_or_else(|error| panic!("extract: {error}"));
        assert!(bridge.jar.is_file());
        assert_eq!(
            extract_bridge(temp.path()).unwrap_or_else(|error| panic!("extract: {error}")),
            bridge
        );
    }
    #[test]
    fn rejects_system_gradle_without_approval() {
        assert!(matches!(
            select_gradle(None, false),
            Err(EmbeddedBridgeError::WrapperRequired)
        ));
    }
}
