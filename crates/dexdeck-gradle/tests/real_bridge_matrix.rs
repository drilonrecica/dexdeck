use std::time::Duration;

use dexdeck_core::{ProcessSupervisor, SecretRedactor};
use dexdeck_gradle::{BridgeRunner, discover_project};
use dexdeck_test_support::{AndroidFixture, COMPATIBILITY_LANES};
use tokio_util::sync::CancellationToken;

/// CI opts in after restoring Gradle/AGP caches and the pinned Java 17 toolchain.
#[tokio::test]
async fn models_executable_agp_matrix() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("DEXDECK_REAL_AGP_TESTS").is_none() {
        return Ok(());
    }
    let requested_lane = std::env::var("DEXDECK_AGP_LANE").ok();
    let requested_fixture = std::env::var("DEXDECK_FIXTURE").ok();
    let directory = tempfile::tempdir()?;
    let runner = BridgeRunner::new(
        directory.path().join("bridge-cache"),
        ProcessSupervisor::new(4 * 1024 * 1024, Duration::from_secs(3))?,
    );
    let fixtures = [
        AndroidFixture::KotlinSingleApp,
        AndroidFixture::GroovySingleApp,
        AndroidFixture::MultiModule,
        AndroidFixture::MultiApp,
        AndroidFixture::Flavors,
        AndroidFixture::DisabledVariant,
        AndroidFixture::Library,
        AndroidFixture::ConventionPlugin,
        AndroidFixture::BuildSrc,
        AndroidFixture::Composite,
        AndroidFixture::CustomTasks,
    ];
    for lane in COMPATIBILITY_LANES.iter().copied().filter(|lane| {
        requested_lane
            .as_ref()
            .is_none_or(|requested| requested == lane.agp)
    }) {
        let lane_root = directory.path().join(lane.agp);
        for fixture in fixtures.into_iter().filter(|fixture| {
            requested_fixture
                .as_ref()
                .is_none_or(|requested| requested == fixture.name())
        }) {
            let root = fixture.write_to_lane(&lane_root, lane)?;
            let discovery = discover_project(&root, true)?;
            let output = runner
                .run(
                    &discovery.root,
                    discovery.wrapper.as_deref(),
                    CancellationToken::new(),
                    CancellationToken::new(),
                    &SecretRedactor::new(),
                )
                .await
                .map_err(|error| format!("{} / AGP {}: {error}", fixture.name(), lane.agp))?;
            assert_eq!(output.model.root, root.canonicalize()?);
            assert!(
                !output.model.modules.is_empty(),
                "{} / AGP {} returned no modules",
                fixture.name(),
                lane.agp
            );
            let detected = output
                .model
                .build
                .agp_version
                .as_deref()
                .unwrap_or_default();
            assert_eq!(
                detected.trim_end_matches(".0"),
                lane.agp.trim_end_matches(".0"),
                "{}",
                fixture.name()
            );
            match fixture {
                AndroidFixture::MultiModule
                | AndroidFixture::Library
                | AndroidFixture::MultiApp => assert!(output.model.modules.len() >= 2),
                AndroidFixture::Flavors => {
                    let app = output
                        .model
                        .modules
                        .iter()
                        .find(|module| module.path == ":app")
                        .ok_or("missing app module")?;
                    assert_eq!(app.product_flavors.len(), 2);
                    assert!(
                        app.variants
                            .iter()
                            .any(|variant| !variant.flavors.is_empty())
                    );
                }
                AndroidFixture::DisabledVariant => assert!(
                    output
                        .model
                        .modules
                        .iter()
                        .flat_map(|module| &module.variants)
                        .any(|variant| !variant.enabled)
                ),
                AndroidFixture::CustomTasks => assert!(
                    output
                        .model
                        .tasks
                        .iter()
                        .any(|task| task.name == "dexdeckFixtureTask")
                ),
                _ => {}
            }
        }
    }
    Ok(())
}

#[test]
fn exact_lane_pairings_are_stable() {
    assert_eq!(
        COMPATIBILITY_LANES
            .iter()
            .map(|lane| (lane.agp, lane.gradle))
            .collect::<Vec<_>>(),
        [
            ("8.0.2", "8.0"),
            ("8.13.0", "8.13"),
            ("9.0.1", "9.1"),
            ("9.3.0", "9.5"),
        ]
    );
}
