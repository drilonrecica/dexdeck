use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectDiscovery {
    pub root: PathBuf,
    pub settings_file: Option<PathBuf>,
    pub wrapper: Option<PathBuf>,
    pub android_signals: Vec<PathBuf>,
    pub sdk_hint: Option<PathBuf>,
    pub java_home: Option<PathBuf>,
}

impl ProjectDiscovery {
    #[must_use]
    pub fn is_android(&self) -> bool {
        !self.android_signals.is_empty()
    }

    #[must_use]
    pub fn has_wrapper(&self) -> bool {
        self.wrapper.is_some()
    }
}

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("project path does not exist: {0}")]
    Missing(PathBuf),
    #[error("no Gradle project found from {0}")]
    NotGradle(PathBuf),
    #[error("failed to inspect {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Discover the nearest Gradle root without starting Gradle.
pub fn discover_project(start: &Path, explicit: bool) -> Result<ProjectDiscovery, DiscoveryError> {
    if !start.exists() {
        return Err(DiscoveryError::Missing(start.to_path_buf()));
    }
    let start = start.canonicalize().map_err(|source| DiscoveryError::Io {
        path: start.to_path_buf(),
        source,
    })?;
    let first = if start.is_file() {
        start.parent().unwrap_or(&start)
    } else {
        &start
    };
    let candidates: Box<dyn Iterator<Item = &Path>> = if explicit {
        Box::new(std::iter::once(first))
    } else {
        Box::new(first.ancestors())
    };
    for candidate in candidates {
        if is_gradle_root(candidate) {
            return inspect(candidate);
        }
    }
    Err(DiscoveryError::NotGradle(start))
}

fn is_gradle_root(path: &Path) -> bool {
    [
        "settings.gradle.kts",
        "settings.gradle",
        "gradlew",
        "gradlew.bat",
    ]
    .iter()
    .any(|name| path.join(name).is_file())
        || (["build.gradle.kts", "build.gradle"]
            .iter()
            .any(|name| path.join(name).is_file())
            && path.join("gradle").is_dir())
}

fn inspect(root: &Path) -> Result<ProjectDiscovery, DiscoveryError> {
    let settings_file = first_file(root, &["settings.gradle.kts", "settings.gradle"]);
    let wrapper = if cfg!(windows) {
        first_file(root, &["gradlew.bat", "gradlew"])
    } else {
        first_file(root, &["gradlew", "gradlew.bat"])
    };
    let mut android_signals = Vec::new();
    collect_named(root, "AndroidManifest.xml", 5, &mut android_signals)?;
    for name in ["build.gradle.kts", "build.gradle"] {
        let file = root.join(name);
        if contains_android_plugin(&file)? {
            android_signals.push(file);
        }
    }
    let sdk_hint = read_sdk_dir(&root.join("local.properties"))
        .or_else(|| env::var_os("ANDROID_HOME").map(PathBuf::from))
        .or_else(|| env::var_os("ANDROID_SDK_ROOT").map(PathBuf::from));
    Ok(ProjectDiscovery {
        root: root.to_path_buf(),
        settings_file,
        wrapper,
        android_signals,
        sdk_hint,
        java_home: env::var_os("JAVA_HOME").map(PathBuf::from),
    })
}

fn first_file(root: &Path, names: &[&str]) -> Option<PathBuf> {
    names
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn contains_android_plugin(path: &Path) -> Result<bool, DiscoveryError> {
    if !path.is_file() {
        return Ok(false);
    }
    let source = fs::read_to_string(path).map_err(|source| DiscoveryError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(source.contains("com.android.application") || source.contains("com.android.library"))
}

fn collect_named(
    root: &Path,
    name: &str,
    depth: usize,
    found: &mut Vec<PathBuf>,
) -> Result<(), DiscoveryError> {
    if depth == 0 {
        return Ok(());
    }
    let entries = fs::read_dir(root).map_err(|source| DiscoveryError::Io {
        path: root.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| DiscoveryError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            found.push(path);
            continue;
        }
        if path.is_dir()
            && !matches!(
                path.file_name().and_then(|v| v.to_str()),
                Some(".git" | ".gradle" | "build")
            )
        {
            collect_named(&path, name, depth - 1, found)?;
        }
    }
    Ok(())
}

fn read_sdk_dir(path: &Path) -> Option<PathBuf> {
    let text = fs::read_to_string(path).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("sdk.dir="))
        .map(|value| PathBuf::from(value.replace("\\\\", "\\")))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn walks_up_and_detects_kotlin_android_project() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("settings.gradle.kts"), "").expect("settings");
        fs::write(
            temp.path().join("build.gradle.kts"),
            "plugins { id(\"com.android.application\") }",
        )
        .expect("build");
        fs::create_dir_all(temp.path().join("app/src/main")).expect("dirs");
        fs::write(
            temp.path().join("app/src/main/AndroidManifest.xml"),
            "<manifest />",
        )
        .expect("manifest");
        let result = discover_project(&temp.path().join("app/src"), false).expect("discovery");
        assert_eq!(result.root, temp.path().canonicalize().expect("canonical"));
        assert!(result.is_android());
    }

    #[test]
    fn explicit_non_root_is_not_walked_up() {
        let temp = tempfile::tempdir().expect("tempdir");
        fs::write(temp.path().join("settings.gradle"), "").expect("settings");
        fs::create_dir(temp.path().join("nested")).expect("nested");
        assert!(matches!(
            discover_project(&temp.path().join("nested"), true),
            Err(DiscoveryError::NotGradle(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn canonicalizes_symlinked_roots() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().expect("tempdir");
        let root = temp.path().join("root");
        fs::create_dir(&root).expect("root");
        fs::write(root.join("settings.gradle"), "").expect("settings");
        let link = temp.path().join("link");
        symlink(&root, &link).expect("symlink");
        assert_eq!(
            discover_project(&link, true).expect("discovery").root,
            root.canonicalize().expect("canonical")
        );
    }
}
