use std::path::PathBuf;

use dexdeck_protocol::{
    AndroidModule, BridgeComplete, BridgeEnvelope, BridgePayload, BridgeProtocolError,
    BridgeStreamValidator, BuildInfo, CliEnvelope, CliEvent, JobId, JobKind, ModelFreshness,
    ModuleKind, ModulesSnapshot, ProjectModel, ProjectSnapshot, ProjectSupport, VariantsSnapshot,
};

#[test]
fn project_snapshot_matches_golden_contract() -> Result<(), serde_json::Error> {
    let model = ProjectModel {
        root: PathBuf::from("/project"),
        build: BuildInfo {
            root: PathBuf::from("/project"),
            gradle_version: "9.5.0".into(),
            agp_version: Some("9.3.0".into()),
            java_version: Some("17".into()),
            kotlin_plugin_version: None,
        },
        included_builds: Vec::new(),
        modules: vec![AndroidModule {
            path: ":app".into(),
            build_id: "root".into(),
            kind: ModuleKind::Application,
            namespace: Some("com.example.app".into()),
            compile_sdk: Some(37),
            target_sdk: Some(37),
            minimum_sdk: Some(23),
            flavor_dimensions: Vec::new(),
            product_flavors: Vec::new(),
            build_types: Vec::new(),
            variants: Vec::new(),
        }],
        tasks: Vec::new(),
        diagnostics: Vec::new(),
    };

    let actual = serde_json::to_string_pretty(&CliEnvelope::new(ProjectSnapshot {
        freshness: ModelFreshness::Current,
        support: ProjectSupport::Full,
        degraded_reason: None,
        project: model,
    }))?;
    assert_eq!(
        format!("{actual}\n"),
        include_str!("golden/project-snapshot.json")
    );
    Ok(())
}

#[test]
fn list_snapshots_match_golden_contracts() -> Result<(), serde_json::Error> {
    let modules = serde_json::to_string_pretty(&CliEnvelope::new(ModulesSnapshot {
        freshness: ModelFreshness::Current,
        support: ProjectSupport::Full,
        degraded_reason: None,
        modules: Vec::new(),
    }))?;
    assert_eq!(
        format!("{modules}\n"),
        include_str!("golden/modules-snapshot.json")
    );
    let variants = serde_json::to_string_pretty(&CliEnvelope::new(VariantsSnapshot {
        freshness: ModelFreshness::Current,
        support: ProjectSupport::Full,
        degraded_reason: None,
        variants: Vec::new(),
    }))?;
    assert_eq!(
        format!("{variants}\n"),
        include_str!("golden/variants-snapshot.json")
    );
    Ok(())
}

#[test]
fn cli_event_matches_golden_contract() -> Result<(), serde_json::Error> {
    let event = CliEnvelope::new(CliEvent::JobStarted {
        job_id: JobId("job-1".into()),
        kind: JobKind::Build,
    });
    let actual = serde_json::to_string(&event)?;
    assert_eq!(actual, include_str!("golden/job-started.jsonl").trim());
    Ok(())
}

#[test]
fn bridge_requires_matching_version_and_completion() -> Result<(), BridgeProtocolError> {
    let build = BridgeEnvelope::new(BridgePayload::Build {
        build: BuildInfo {
            root: PathBuf::from("/project"),
            gradle_version: "9.5.0".into(),
            agp_version: Some("9.3.0".into()),
            java_version: Some("17".into()),
            kotlin_plugin_version: None,
        },
    });
    let complete = BridgeEnvelope::new(BridgePayload::Complete {
        complete: BridgeComplete {
            duration_ms: 10,
            record_count: 1,
            model_hash: "abc123".into(),
        },
    });

    let mut validator = BridgeStreamValidator::default();
    validator.accept(&build)?;
    validator.accept(&complete)?;
    assert_eq!(validator.finish()?.record_count, 1);

    let mut unsupported = build;
    unsupported.protocol_version = 2;
    let mut validator = BridgeStreamValidator::default();
    assert_eq!(
        validator.accept(&unsupported),
        Err(BridgeProtocolError::UnsupportedVersion {
            expected: 1,
            found: 2,
        })
    );
    Ok(())
}

#[test]
fn bridge_rejects_incomplete_and_trailing_streams() -> Result<(), BridgeProtocolError> {
    assert_eq!(
        BridgeStreamValidator::default().finish(),
        Err(BridgeProtocolError::MissingCompletion)
    );

    let completion = BridgeEnvelope::new(BridgePayload::Complete {
        complete: BridgeComplete {
            duration_ms: 0,
            record_count: 0,
            model_hash: "empty".into(),
        },
    });
    let mut validator = BridgeStreamValidator::default();
    validator.accept(&completion)?;
    assert_eq!(
        validator.accept(&completion),
        Err(BridgeProtocolError::DuplicateCompletion)
    );
    Ok(())
}
