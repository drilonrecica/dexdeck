use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    ConfigError, ConfigLayer, ConfigResolver, ConfigWarning, ParsedConfig, ResolvedConfig,
    StorageError, parse_config,
};

/// Inputs are listed from lowest to highest precedence by `load` explicitly.
#[derive(Clone, Debug, Default)]
pub struct ConfigSources {
    pub detected: Option<ConfigLayer>,
    pub shared: Option<PathBuf>,
    pub user: Option<PathBuf>,
    pub explicit: Option<PathBuf>,
    pub cli: Option<ConfigLayer>,
}

#[derive(Clone, Debug)]
pub struct LoadedConfig {
    pub resolved: ResolvedConfig,
    pub documents: Vec<ParsedConfig>,
    pub warnings: Vec<ConfigWarning>,
}

#[derive(Clone, Debug, Default)]
pub struct ConfigLoader;

impl ConfigLoader {
    pub fn load(&self, sources: &ConfigSources) -> Result<LoadedConfig, ConfigError> {
        let mut resolver = ConfigResolver::new();
        let mut documents = Vec::new();
        let mut warnings = Vec::new();

        if let Some(detected) = &sources.detected {
            resolver.apply(detected, "<detected>")?;
        }
        self.apply_optional(
            &mut resolver,
            &mut documents,
            &mut warnings,
            sources.shared.as_deref(),
            false,
        )?;
        self.apply_optional(
            &mut resolver,
            &mut documents,
            &mut warnings,
            sources.user.as_deref(),
            false,
        )?;
        self.apply_optional(
            &mut resolver,
            &mut documents,
            &mut warnings,
            sources.explicit.as_deref(),
            true,
        )?;
        if let Some(cli) = &sources.cli {
            resolver.apply(cli, "<cli>")?;
        }

        Ok(LoadedConfig {
            resolved: resolver.finish(),
            documents,
            warnings,
        })
    }

    fn apply_optional(
        &self,
        resolver: &mut ConfigResolver,
        documents: &mut Vec<ParsedConfig>,
        warnings: &mut Vec<ConfigWarning>,
        path: Option<&Path>,
        required: bool,
    ) -> Result<(), ConfigError> {
        let Some(path) = path else {
            return Ok(());
        };
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => return Err(StorageError::io(path, error).into()),
        };
        let parsed = parse_config(path, &source)?;
        resolver.apply(
            &ConfigLayer::from(parsed.config.clone()),
            path.to_path_buf(),
        )?;
        warnings.extend(parsed.warnings.iter().cloned());
        documents.push(parsed);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GradleConfig, ProjectConfig};

    #[test]
    fn applies_documented_precedence() -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let shared = directory.path().join("shared.toml");
        let user = directory.path().join("user.toml");
        fs::write(
            &shared,
            "schema_version = 1\n[project]\ndefault_module = \":shared\"\n",
        )?;
        fs::write(
            &user,
            "schema_version = 1\n[project]\ndefault_module = \":user\"\n",
        )?;
        let loaded = ConfigLoader.load(&ConfigSources {
            detected: Some(ConfigLayer {
                project: ProjectConfig {
                    default_module: Some(":detected".into()),
                    default_variant: None,
                },
                ..ConfigLayer::default()
            }),
            shared: Some(shared),
            user: Some(user),
            cli: Some(ConfigLayer {
                gradle: GradleConfig {
                    arguments: Some(vec!["--offline".into()]),
                },
                ..ConfigLayer::default()
            }),
            ..ConfigSources::default()
        })?;
        assert_eq!(
            loaded.resolved.project.default_module.as_deref(),
            Some(":user")
        );
        assert_eq!(loaded.resolved.gradle_arguments, ["--offline"]);
        Ok(())
    }
}
