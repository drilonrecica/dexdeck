use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::{ProjectIdentity, StorageError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoragePaths {
    config_root: PathBuf,
    cache_root: PathBuf,
}

impl StoragePaths {
    pub fn discover() -> Result<Self, StorageError> {
        let directories = ProjectDirs::from("", "", "dexdeck")
            .ok_or(StorageError::PlatformDirectoriesUnavailable)?;
        Ok(Self {
            config_root: directories.config_dir().to_path_buf(),
            cache_root: directories.cache_dir().to_path_buf(),
        })
    }

    #[must_use]
    pub fn from_roots(config_root: PathBuf, cache_root: PathBuf) -> Self {
        Self {
            config_root,
            cache_root,
        }
    }

    #[must_use]
    pub fn config_root(&self) -> &Path {
        &self.config_root
    }

    #[must_use]
    pub fn cache_root(&self) -> &Path {
        &self.cache_root
    }

    #[must_use]
    pub fn bridge_cache_root(&self) -> PathBuf {
        self.cache_root.join("bridge")
    }

    #[must_use]
    pub fn project(&self, identity: &ProjectIdentity) -> ProjectPaths {
        let user_project_root = self.config_root.join("projects").join(identity.hash());
        let cache = self.cache_root.join("projects").join(identity.hash());
        ProjectPaths {
            user_config: user_project_root.join("config.toml"),
            model: cache.join("model.json"),
            fingerprint: cache.join("fingerprint.json"),
            session: cache.join("session.json"),
            jobs: cache.join("jobs.json"),
            filters: cache.join("filters.json"),
            trust: cache.join("trust.json"),
            cache,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectPaths {
    pub cache: PathBuf,
    pub user_config: PathBuf,
    pub model: PathBuf,
    pub fingerprint: PathBuf,
    pub session: PathBuf,
    pub jobs: PathBuf,
    pub filters: PathBuf,
    pub trust: PathBuf,
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn project_paths_hide_the_canonical_path() -> Result<(), Box<dyn std::error::Error>> {
        let project = tempdir()?;
        let identity = ProjectIdentity::from_path(project.path())?;
        let paths = StoragePaths::from_roots(PathBuf::from("/config"), PathBuf::from("/cache"));
        let project_paths = paths.project(&identity);

        assert_eq!(
            project_paths.user_config,
            PathBuf::from("/config")
                .join("projects")
                .join(identity.hash())
                .join("config.toml")
        );
        assert_eq!(project_paths.model, project_paths.cache.join("model.json"));
        assert!(
            !project_paths
                .cache
                .to_string_lossy()
                .contains(&project.path().to_string_lossy()[..])
        );
        Ok(())
    }
}
