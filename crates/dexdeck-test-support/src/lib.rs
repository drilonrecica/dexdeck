//! Shared test fixtures and integration helpers.

use std::{
    fs, io,
    path::{Path, PathBuf},
};

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
        let root = parent.join(self.name());
        fs::create_dir_all(root.join("app/src/main"))?;
        let kotlin = !matches!(self, Self::GroovySingleApp);
        let suffix = if kotlin { ".kts" } else { "" };
        let agp = if matches!(self, Self::Agp7Degraded) {
            "7.4.2"
        } else {
            "8.13.0"
        };
        fs::write(
            root.join(format!("settings.gradle{suffix}")),
            "pluginManagement { repositories { google(); mavenCentral(); gradlePluginPortal() } }\nrootProject.name = \"fixture\"\ninclude(\":app\")\n",
        )?;
        fs::write(
            root.join(format!("build.gradle{suffix}")),
            format!(
                "plugins {{ id(\"com.android.application\") version \"{agp}\" apply false }}\n"
            ),
        )?;
        fs::write(
            root.join("app").join(format!("build.gradle{suffix}")),
            "plugins { id(\"com.android.application\") }\nandroid { namespace = \"dev.dexdeck.fixture\"; compileSdk = 36 }\n",
        )?;
        fs::write(
            root.join("app/src/main/AndroidManifest.xml"),
            "<manifest xmlns:android=\"http://schemas.android.com/apk/res/android\"><application /></manifest>\n",
        )?;
        if !matches!(self, Self::MissingWrapper) {
            fs::create_dir_all(root.join("gradle/wrapper"))?;
            fs::write(
                root.join("gradlew"),
                if matches!(self, Self::BrokenWrapper) {
                    "broken\n"
                } else {
                    "#!/bin/sh\nexit 0\n"
                },
            )?;
            fs::write(
                root.join("gradle/wrapper/gradle-wrapper.properties"),
                "distributionUrl=https\\://services.gradle.org/distributions/gradle-8.13-bin.zip\n",
            )?;
        }
        match self {
            Self::MultiModule | Self::Library => fs::create_dir_all(root.join("library/src/main"))?,
            Self::MultiApp => fs::create_dir_all(root.join("admin/src/main"))?,
            Self::ConventionPlugin => fs::create_dir_all(root.join("build-logic/src/main/kotlin"))?,
            Self::BuildSrc => fs::create_dir_all(root.join("buildSrc/src/main/kotlin"))?,
            Self::Composite => fs::create_dir_all(root.join("included-build"))?,
            Self::Flavors
            | Self::DisabledVariant
            | Self::CustomTasks
            | Self::KotlinSingleApp
            | Self::GroovySingleApp
            | Self::BrokenWrapper
            | Self::MissingWrapper
            | Self::Agp7Degraded => {}
        }
        Ok(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_matrix_materializes_every_shape() {
        let directory = tempfile::tempdir().expect("tempdir");
        for fixture in AndroidFixture::ALL {
            let root = fixture.write_to(directory.path()).expect("fixture");
            assert!(root.join("app/src/main/AndroidManifest.xml").is_file());
        }
        assert_eq!(
            AGP_COMPATIBILITY_LANES,
            ["8.0.2", "8.13.0", "9.0.1", "9.3.0"]
        );
    }
}
