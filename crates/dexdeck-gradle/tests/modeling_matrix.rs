use std::{fs, path::PathBuf, process::Command};

use dexdeck_config::{discover_model_inputs, fingerprint};
use dexdeck_gradle::{
    AdapterKind, EmbeddedBridgeError, ModelRefresh, discover_project, parse_complete_output,
    select_adapter,
};
use dexdeck_protocol::{
    BRIDGE_PROTOCOL_VERSION, BridgeComplete, BridgeEnvelope, BridgePayload, BuildInfo, ProjectModel,
};
use dexdeck_test_support::{AGP_COMPATIBILITY_LANES, AndroidFixture};

#[test]
fn discovers_every_fixture_without_mutating_it() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    for fixture in AndroidFixture::ALL {
        let root = fixture.write_to(temp.path())?;
        let before = file_inventory(&root)?;
        let discovery = discover_project(&root.join("app/src/main"), false)?;
        assert_eq!(discovery.root, root.canonicalize()?);
        assert!(discovery.is_android(), "{}", fixture.name());
        assert_eq!(
            file_inventory(&root)?,
            before,
            "{} was mutated",
            fixture.name()
        );
    }
    Ok(())
}

#[test]
fn compatibility_lanes_select_expected_adapters() -> Result<(), Box<dyn std::error::Error>> {
    let expected = [
        AdapterKind::Agp8,
        AdapterKind::Agp8,
        AdapterKind::Agp9,
        AdapterKind::Agp9,
    ];
    for (version, expected) in AGP_COMPATIBILITY_LANES.iter().zip(expected) {
        assert_eq!(select_adapter(Some(version))?, expected);
    }
    assert_eq!(select_adapter(Some("7.4.2"))?, AdapterKind::Degraded);
    Ok(())
}

#[test]
fn accepts_only_complete_compatible_bridge_streams() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from("/fixture");
    let build = BridgeEnvelope::new(BridgePayload::Build {
        build: BuildInfo {
            root: root.clone(),
            gradle_version: "8.13".into(),
            agp_version: Some("8.13.0".into()),
            java_version: Some("17".into()),
            kotlin_plugin_version: None,
        },
    });
    let complete = BridgeEnvelope::new(BridgePayload::Complete {
        complete: BridgeComplete {
            duration_ms: 1,
            record_count: 1,
            model_hash: "hash".into(),
        },
    });
    let valid = format!(
        "{}\n{}\n",
        serde_json::to_string(&build)?,
        serde_json::to_string(&complete)?
    );
    assert_eq!(parse_complete_output(&valid)?.len(), 2);
    assert!(matches!(
        parse_complete_output(&serde_json::to_string(&build)?),
        Err(EmbeddedBridgeError::Protocol(_))
    ));
    assert!(matches!(
        parse_complete_output("not-json\n"),
        Err(EmbeddedBridgeError::Protocol(_))
    ));
    let incompatible = valid.replacen(
        &format!("\"protocolVersion\":{BRIDGE_PROTOCOL_VERSION}"),
        "\"protocolVersion\":999",
        1,
    );
    assert!(matches!(
        parse_complete_output(&incompatible),
        Err(EmbeddedBridgeError::Protocol(_))
    ));
    Ok(())
}

#[test]
fn fingerprints_invalidate_only_when_model_inputs_change() -> Result<(), Box<dyn std::error::Error>>
{
    let temp = tempfile::tempdir()?;
    fs::write(
        temp.path().join("settings.gradle"),
        "rootProject.name='one'\n",
    )?;
    fs::write(temp.path().join("Main.kt"), "class Main\n")?;
    let inputs = discover_model_inputs(temp.path())?;
    let initial = fingerprint(&inputs, None)?;
    fs::write(temp.path().join("Main.kt"), "class Changed\n")?;
    assert_eq!(fingerprint(&inputs, Some(&initial))?, initial);
    fs::write(
        temp.path().join("settings.gradle"),
        "rootProject.name='changed-name'\n",
    )?;
    assert_ne!(fingerprint(&inputs, Some(&initial))?, initial);
    Ok(())
}

#[test]
fn cancellation_and_failure_never_replace_valid_model() {
    let original = ProjectModel::empty(PathBuf::from("original"));
    let replacement = ProjectModel::empty(PathBuf::from("replacement"));
    let mut refresh = ModelRefresh::new(Some(original.clone()));
    let cancelled = refresh.begin();
    refresh.cancel();
    assert!(!refresh.complete(cancelled, replacement.clone()));
    let failed = refresh.begin();
    refresh.fail(failed, true);
    assert_eq!(refresh.model(), Some(&original));
}

fn file_inventory(root: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    fn visit(
        root: &std::path::Path,
        current: &std::path::Path,
        files: &mut Vec<PathBuf>,
    ) -> std::io::Result<()> {
        for entry in fs::read_dir(current)? {
            let path = entry?.path();
            if path.is_dir() {
                visit(root, &path, files)?;
            } else {
                files.push(path.strip_prefix(root).unwrap_or(&path).to_path_buf());
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    visit(root, root, &mut files)?;
    files.sort();
    Ok(files)
}

#[test]
fn discovery_leaves_git_status_unchanged() -> Result<(), Box<dyn std::error::Error>> {
    let temp = tempfile::tempdir()?;
    let root = AndroidFixture::KotlinSingleApp.write_to(temp.path())?;
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&root)
        .status()?;
    assert!(init.success());
    let before = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()?
        .stdout;
    let _ = discover_project(&root, true)?;
    let after = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&root)
        .output()?
        .stdout;
    assert_eq!(after, before);
    Ok(())
}
