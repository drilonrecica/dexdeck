//! Shared test fixtures and integration helpers.

use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FakeToolScenario {
    pub responses: Vec<FakeToolResponse>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FakeToolResponse {
    pub arguments: Vec<String>,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    #[serde(default)]
    pub exit_code: i32,
    #[serde(default)]
    pub delay_ms: u64,
}

#[derive(Clone, Debug)]
pub struct FakeTool {
    pub executable: PathBuf,
    pub calls: PathBuf,
}

impl FakeTool {
    pub fn install(
        compiled_helper: &Path,
        executable: &Path,
        scenario: &FakeToolScenario,
    ) -> io::Result<Self> {
        if let Some(parent) = executable.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(compiled_helper, executable)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755))?;
        }
        let scenario_path = executable.with_extension("scenario.json");
        let calls = executable.with_extension("calls.jsonl");
        fs::write(
            scenario_path,
            serde_json::to_vec(scenario).map_err(io::Error::other)?,
        )?;
        Ok(Self {
            executable: executable.into(),
            calls,
        })
    }

    pub fn calls(&self) -> io::Result<Vec<Vec<String>>> {
        let source = match fs::read_to_string(&self.calls) {
            Ok(source) => source,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        source
            .lines()
            .map(|line| serde_json::from_str(line).map_err(io::Error::other))
            .collect()
    }
}

/// Android project shapes covered by the project-model compatibility suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AndroidFixture {
    KotlinSingleApp,
    GroovySingleApp,
    MultiModule,
    MultiApp,
    Flavors,
    DisabledVariant,
    Library,
    ConventionPlugin,
    BuildSrc,
    Composite,
    CustomTasks,
    BrokenWrapper,
    MissingWrapper,
    Agp7Degraded,
}

/// Supported AGP compatibility lanes. Patch versions are intentionally pinned.
pub const AGP_COMPATIBILITY_LANES: &[&str] = &["8.0.2", "8.13.0", "9.0.1", "9.3.0"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityLane {
    pub agp: &'static str,
    pub gradle: &'static str,
}

pub const COMPATIBILITY_LANES: &[CompatibilityLane] = &[
    CompatibilityLane {
        agp: "8.0.2",
        gradle: "8.0",
    },
    CompatibilityLane {
        agp: "8.13.0",
        gradle: "8.13",
    },
    CompatibilityLane {
        agp: "9.0.1",
        gradle: "9.1",
    },
    CompatibilityLane {
        agp: "9.3.0",
        gradle: "9.5",
    },
];

const GRADLE_WRAPPER_JAR: &[u8] = include_bytes!("../assets/gradle-wrapper.jar");

impl AndroidFixture {
    pub const ALL: [Self; 14] = [
        Self::KotlinSingleApp,
        Self::GroovySingleApp,
        Self::MultiModule,
        Self::MultiApp,
        Self::Flavors,
        Self::DisabledVariant,
        Self::Library,
        Self::ConventionPlugin,
        Self::BuildSrc,
        Self::Composite,
        Self::CustomTasks,
        Self::BrokenWrapper,
        Self::MissingWrapper,
        Self::Agp7Degraded,
    ];

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::KotlinSingleApp => "kotlin-single-app",
            Self::GroovySingleApp => "groovy-single-app",
            Self::MultiModule => "multi-module",
            Self::MultiApp => "multi-app",
            Self::Flavors => "flavors",
            Self::DisabledVariant => "disabled-variant",
            Self::Library => "library",
            Self::ConventionPlugin => "convention-plugin",
            Self::BuildSrc => "build-src",
            Self::Composite => "composite",
            Self::CustomTasks => "custom-tasks",
            Self::BrokenWrapper => "broken-wrapper",
            Self::MissingWrapper => "missing-wrapper",
            Self::Agp7Degraded => "agp7-degraded",
        }
    }

    /// Materialize a minimal, network-free Gradle fixture.
    pub fn write_to(self, parent: &Path) -> io::Result<PathBuf> {
        self.write_to_lane(parent, COMPATIBILITY_LANES[1])
    }

    /// Materialize an executable project for one pinned compatibility lane.
    pub fn write_to_lane(self, parent: &Path, lane: CompatibilityLane) -> io::Result<PathBuf> {
        let root = parent.join(self.name());
        fs::create_dir_all(root.join("app/src/main"))?;
        let kotlin = !matches!(self, Self::GroovySingleApp);
        let suffix = if kotlin { ".kts" } else { "" };
        let agp = if matches!(self, Self::Agp7Degraded) {
            "7.4.2"
        } else {
            lane.agp
        };
        let settings = if kotlin {
            "pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS); repositories { google(); mavenCentral() } }\nrootProject.name = \"fixture\"\ninclude(\":app\")\n"
        } else {
            "pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS); repositories { google(); mavenCentral() } }\nrootProject.name = 'fixture'\ninclude ':app'\n"
        };
        fs::write(root.join(format!("settings.gradle{suffix}")), settings)?;
        fs::write(
            root.join(format!("build.gradle{suffix}")),
            if kotlin {
                format!(
                    "plugins {{ id(\"com.android.application\") version \"{agp}\" apply false; id(\"com.android.library\") version \"{agp}\" apply false }}\n"
                )
            } else {
                format!(
                    "plugins {{ id 'com.android.application' version '{agp}' apply false; id 'com.android.library' version '{agp}' apply false }}\n"
                )
            },
        )?;
        fs::write(
            root.join("app").join(format!("build.gradle{suffix}")),
            if kotlin {
                "plugins { id(\"com.android.application\") }\nandroid { namespace = \"dev.dexdeck.fixture\"; compileSdk = 36 }\n"
            } else {
                "plugins { id 'com.android.application' }\nandroid { namespace 'dev.dexdeck.fixture'; compileSdk 36 }\n"
            },
        )?;
        fs::write(
            root.join("app/src/main/AndroidManifest.xml"),
            "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\"><application /></manifest>\n",
        )?;
        if !matches!(self, Self::MissingWrapper) {
            let distribution_gradle = if lane.gradle.starts_with("9.") {
                format!("{}.0", lane.gradle)
            } else {
                lane.gradle.to_owned()
            };
            fs::create_dir_all(root.join("gradle/wrapper"))?;
            fs::write(
                root.join("gradlew"),
                if matches!(self, Self::BrokenWrapper) {
                    "broken\n"
                } else {
                    "#!/bin/sh\nAPP_HOME=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nexec java -classpath \"$APP_HOME/gradle/wrapper/gradle-wrapper.jar\" org.gradle.wrapper.GradleWrapperMain \"$@\"\n"
                },
            )?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(root.join("gradlew"), fs::Permissions::from_mode(0o755))?;
            }
            fs::write(
                root.join("gradlew.bat"),
                "@echo off\r\njava -classpath \"%~dp0\\gradle\\wrapper\\gradle-wrapper.jar\" org.gradle.wrapper.GradleWrapperMain %*\r\n",
            )?;
            fs::write(
                root.join("gradle/wrapper/gradle-wrapper.jar"),
                GRADLE_WRAPPER_JAR,
            )?;
            fs::write(
                root.join("gradle/wrapper/gradle-wrapper.properties"),
                format!(
                    "distributionBase=GRADLE_USER_HOME\ndistributionPath=wrapper/dists\ndistributionUrl=https\\://services.gradle.org/distributions/gradle-{distribution_gradle}-bin.zip\nnetworkTimeout=10000\nvalidateDistributionUrl=true\nzipStoreBase=GRADLE_USER_HOME\nzipStorePath=wrapper/dists\n"
                ),
            )?;
        }
        match self {
            Self::MultiModule | Self::Library => {
                fs::create_dir_all(root.join("library/src/main"))?;
                fs::write(
                    root.join(format!("settings.gradle{suffix}")),
                    format!("{settings}include(\":library\")\n"),
                )?;
                fs::write(
                    root.join("library/build.gradle.kts"),
                    "plugins { id(\"com.android.library\") }\nandroid { namespace = \"dev.dexdeck.library\"; compileSdk = 36 }\n",
                )?;
                fs::write(
                    root.join("library/src/main/AndroidManifest.xml"),
                    "<manifest />\n",
                )?;
            }
            Self::MultiApp => {
                fs::create_dir_all(root.join("admin/src/main"))?;
                fs::write(
                    root.join(format!("settings.gradle{suffix}")),
                    format!("{settings}include(\":admin\")\n"),
                )?;
                fs::write(
                    root.join("admin/build.gradle.kts"),
                    "plugins { id(\"com.android.application\") }\nandroid { namespace = \"dev.dexdeck.admin\"; compileSdk = 36 }\n",
                )?;
                fs::write(
                    root.join("admin/src/main/AndroidManifest.xml"),
                    "<manifest />\n",
                )?;
            }
            Self::ConventionPlugin => {
                fs::write(
                    root.join("settings.gradle.kts"),
                    "pluginManagement { includeBuild(\"build-logic\"); repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS); repositories { google(); mavenCentral() } }\nrootProject.name = \"fixture\"\ninclude(\":app\")\n",
                )?;
                fs::write(
                    root.join("app/build.gradle.kts"),
                    "plugins { id(\"dev.dexdeck.android-app\") }\nandroid { namespace = \"dev.dexdeck.fixture\"; compileSdk = 36 }\n",
                )?;
                fs::create_dir_all(root.join("build-logic/src/main/java/dev/dexdeck/fixture"))?;
                fs::write(
                    root.join("build-logic/settings.gradle.kts"),
                    "pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\ndependencyResolutionManagement { repositories { google(); mavenCentral() } }\nrootProject.name = \"build-logic\"\n",
                )?;
                fs::write(
                    root.join("build-logic/build.gradle.kts"),
                    format!(
                        "plugins {{ `java-gradle-plugin` }}\nrepositories {{ google(); mavenCentral() }}\ndependencies {{ implementation(\"com.android.tools.build:gradle:{agp}\") }}\ngradlePlugin {{ plugins {{ create(\"androidApp\") {{ id = \"dev.dexdeck.android-app\"; implementationClass = \"dev.dexdeck.fixture.AndroidAppPlugin\" }} }} }}\n"
                    ),
                )?;
                fs::write(
                    root.join(
                        "build-logic/src/main/java/dev/dexdeck/fixture/AndroidAppPlugin.java",
                    ),
                    "package dev.dexdeck.fixture;\n\nimport org.gradle.api.Plugin;\nimport org.gradle.api.Project;\n\npublic final class AndroidAppPlugin implements Plugin<Project> {\n    @Override public void apply(Project project) {\n        project.getPluginManager().apply(\"com.android.application\");\n    }\n}\n",
                )?;
            }
            Self::BuildSrc => {
                fs::create_dir_all(root.join("buildSrc/src/main/kotlin"))?;
                fs::write(
                    root.join("buildSrc/build.gradle.kts"),
                    "plugins { `kotlin-dsl` }\n",
                )?;
            }
            Self::Composite => {
                fs::create_dir_all(root.join("included-build"))?;
                fs::write(
                    root.join("included-build/settings.gradle.kts"),
                    "rootProject.name = \"included\"\n",
                )?;
                append(
                    &root.join(format!("settings.gradle{suffix}")),
                    "includeBuild(\"included-build\")\n",
                )?;
            }
            Self::Flavors => append(
                &root.join("app/build.gradle.kts"),
                "android { flavorDimensions += \"tier\"; productFlavors { create(\"free\") { dimension = \"tier\" }; create(\"paid\") { dimension = \"tier\" } } }\n",
            )?,
            Self::DisabledVariant => append(
                &root.join("app/build.gradle.kts"),
                "androidComponents { beforeVariants(selector().withBuildType(\"release\")) { it.enable = false } }\n",
            )?,
            Self::CustomTasks => append(
                &root.join("build.gradle.kts"),
                "tasks.register(\"dexdeckFixtureTask\")\n",
            )?,
            Self::KotlinSingleApp
            | Self::GroovySingleApp
            | Self::BrokenWrapper
            | Self::MissingWrapper
            | Self::Agp7Degraded => {}
        }
        Ok(root)
    }
}

fn append(path: &Path, content: &str) -> io::Result<()> {
    let mut source = fs::read_to_string(path)?;
    source.push_str(content);
    fs::write(path, source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_matrix_materializes_every_shape() -> std::io::Result<()> {
        let directory = tempfile::tempdir()?;
        for fixture in AndroidFixture::ALL {
            let root = fixture.write_to(directory.path())?;
            assert!(root.join("app/src/main/AndroidManifest.xml").is_file());
        }
        assert_eq!(
            AGP_COMPATIBILITY_LANES,
            ["8.0.2", "8.13.0", "9.0.1", "9.3.0"]
        );
        assert_eq!(COMPATIBILITY_LANES[3].gradle, "9.5");
        Ok(())
    }
}
