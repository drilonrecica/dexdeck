use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use toml_edit::{DocumentMut, Item, TableLike};

use crate::{ConfigError, ConfigFile, ConfigWarning, StorageError, write_text_atomic};

#[derive(Clone, Debug)]
pub struct ParsedConfig {
    pub config: ConfigFile,
    pub document: ConfigDocument,
    pub warnings: Vec<ConfigWarning>,
}

#[derive(Clone, Debug)]
pub struct ConfigDocument {
    source: String,
    document: DocumentMut,
}

impl ConfigDocument {
    #[must_use]
    pub fn as_document(&self) -> &DocumentMut {
        &self.document
    }

    #[must_use]
    pub fn render(&self) -> String {
        self.document.to_string()
    }

    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigScope {
    Local,
    Shared,
}

pub fn parse_config(path: impl AsRef<Path>, input: &str) -> Result<ParsedConfig, ConfigError> {
    let path = path.as_ref();
    let document = input
        .parse::<DocumentMut>()
        .map_err(|error| parse_error(path, input, error.message(), error.span()))?;
    let config = toml_edit::de::from_document::<ConfigFile>(document.clone())
        .map_err(|error| parse_error(path, input, error.message(), error.span()))?;
    if let Err(error) = config.validate(path.to_path_buf()) {
        return Err(attach_validation_span(error, input, &document));
    }
    let mut warnings = collect_warnings(input, &document);
    if config
        .logcat
        .as_ref()
        .and_then(|logcat| logcat.buffer_mib)
        .is_some_and(|buffer_mib| buffer_mib > 256)
    {
        let item = document
            .get("logcat")
            .and_then(Item::as_table_like)
            .and_then(|table| table.get("buffer_mib"));
        let (line, column) = line_column(
            input,
            item.and_then(Item::span).map_or(0, |span| span.start),
        );
        warnings.push(ConfigWarning {
            path: "logcat.buffer_mib".into(),
            line,
            column,
            message: "Logcat buffers above 256 MiB may materially increase memory use".into(),
        });
    }

    Ok(ParsedConfig {
        config,
        document: ConfigDocument {
            source: input.to_owned(),
            document,
        },
        warnings,
    })
}

fn attach_validation_span(error: ConfigError, input: &str, document: &DocumentMut) -> ConfigError {
    let ConfigError::Validation {
        path,
        field,
        message,
    } = error
    else {
        return error;
    };
    let mut item = None;
    let mut table: &dyn TableLike = document.as_table();
    for (index, component) in field.split('.').enumerate() {
        let found = table.get(component);
        item = found;
        if index + 1 < field.split('.').count() {
            let Some(next) = found.and_then(Item::as_table_like) else {
                break;
            };
            table = next;
        }
    }
    let (line, column) = line_column(
        input,
        item.and_then(Item::span).map_or(0, |span| span.start),
    );
    ConfigError::ValidationAt {
        path,
        field,
        line,
        column,
        message,
    }
}

pub fn write_config_document(
    path: impl AsRef<Path>,
    document: &ConfigDocument,
) -> Result<(), ConfigError> {
    write_text_atomic(path, &document.render()).map_err(ConfigError::from)
}

pub fn write_config_migration(
    path: impl AsRef<Path>,
    document: &ConfigDocument,
    scope: ConfigScope,
    shared_confirmed: bool,
) -> Result<Option<PathBuf>, ConfigError> {
    let path = path.as_ref();
    if scope == ConfigScope::Shared && !shared_confirmed {
        return Err(ConfigError::SharedMigrationConfirmationRequired {
            path: path.to_path_buf(),
        });
    }

    let backup = if path.exists() {
        let backup = path.with_extension("toml.bak");
        let source = fs::read_to_string(path)
            .map_err(|error| ConfigError::Storage(StorageError::io(path, error)))?;
        write_text_atomic(&backup, &source)?;
        Some(backup)
    } else {
        None
    };
    write_config_document(path, document)?;
    Ok(backup)
}

fn parse_error(
    path: &Path,
    input: &str,
    message: &str,
    span: Option<std::ops::Range<usize>>,
) -> ConfigError {
    let (line, column) = line_column(input, span.map_or(0, |range| range.start));
    ConfigError::Parse {
        path: path.to_path_buf(),
        line,
        column,
        message: message.to_owned(),
    }
}

fn collect_warnings(input: &str, document: &DocumentMut) -> Vec<ConfigWarning> {
    let mut warnings = Vec::new();
    check_table(
        input,
        document.as_table(),
        "",
        &set(&[
            "schema_version",
            "project",
            "gradle",
            "ui",
            "logcat",
            "editor",
            "profiles",
            "commands",
        ]),
        &mut warnings,
    );
    check_named_section(
        input,
        document.get("project"),
        "project",
        &["default_module", "default_variant"],
        &mut warnings,
    );
    check_named_section(
        input,
        document.get("gradle"),
        "gradle",
        &["arguments"],
        &mut warnings,
    );
    check_named_section(
        input,
        document.get("ui"),
        "ui",
        &["keymap", "reduced_motion", "unicode"],
        &mut warnings,
    );
    check_named_section(
        input,
        document.get("logcat"),
        "logcat",
        &["buffer_mib", "minimum_priority", "default_scope"],
        &mut warnings,
    );
    check_named_section(
        input,
        document.get("editor"),
        "editor",
        &["command"],
        &mut warnings,
    );
    check_dynamic_sections(
        input,
        document.get("profiles"),
        "profiles",
        &[
            "module",
            "variant",
            "device",
            "launch_mode",
            "launcher_activity",
            "activity",
            "deep_link",
            "intent_action",
            "intent_categories",
            "intent_extras",
            "gradle_properties",
            "environment",
            "start_emulator_if_offline",
        ],
        &mut warnings,
    );
    check_dynamic_sections(
        input,
        document.get("commands"),
        "commands",
        &["command", "working_directory", "environment"],
        &mut warnings,
    );
    warnings
}

fn check_named_section(
    input: &str,
    item: Option<&Item>,
    path: &str,
    allowed: &[&str],
    warnings: &mut Vec<ConfigWarning>,
) {
    if let Some(table) = item.and_then(Item::as_table_like) {
        check_table(input, table, path, &set(allowed), warnings);
    }
}

fn check_dynamic_sections(
    input: &str,
    item: Option<&Item>,
    path: &str,
    allowed: &[&str],
    warnings: &mut Vec<ConfigWarning>,
) {
    let Some(table) = item.and_then(Item::as_table_like) else {
        return;
    };
    let allowed = set(allowed);
    for (name, item) in table.iter() {
        if let Some(section) = item.as_table_like() {
            check_table(
                input,
                section,
                &format!("{path}.{name}"),
                &allowed,
                warnings,
            );
        }
    }
}

fn check_table(
    input: &str,
    table: &dyn TableLike,
    prefix: &str,
    allowed: &BTreeSet<&str>,
    warnings: &mut Vec<ConfigWarning>,
) {
    for (key, item) in table.iter() {
        if allowed.contains(key) {
            continue;
        }
        let path = if prefix.is_empty() {
            key.to_owned()
        } else {
            format!("{prefix}.{key}")
        };
        let (line, column) = line_column(input, item.span().map_or(0, |span| span.start));
        warnings.push(ConfigWarning {
            path: path.clone(),
            line,
            column,
            message: format!("unknown configuration field {path}"),
        });
    }
}

fn set<'a>(values: &'a [&'a str]) -> BTreeSet<&'a str> {
    values.iter().copied().collect()
}

fn line_column(input: &str, offset: usize) -> (usize, usize) {
    let prefix = &input[..offset.min(input.len())];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix
        .rsplit_once('\n')
        .map_or(prefix.len() + 1, |(_, suffix)| suffix.len() + 1);
    (line, column)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::{ConfigLayer, ConfigResolver, EnvironmentValue, KeymapPreset, UnicodeMode};

    const EXAMPLE: &str = r#"# shared configuration
schema_version = 1

[project]
default_module = ":app"
default_variant = "demoDebug"

[gradle]
arguments = ["--stacktrace"]

[ui]
keymap = "default"
reduced_motion = false
unicode = "auto"

[logcat]
buffer_mib = 32
minimum_priority = "debug"
default_scope = "application"

[editor]
command = ["zed", "{path}:{line}:{column}"]

[profiles.demo]
module = ":app"
variant = "demoDebug"
device = "last-used"
launch_mode = "launcher"

[profiles.local-backend.environment]
API_BASE_URL = "http://localhost:8080"
API_TOKEN = { from_env = "DEMO_API_TOKEN" }

[commands.mock-server]
command = ["docker", "compose", "up", "mock-api"]
working_directory = "."
"#;

    #[test]
    fn parses_the_documented_configuration() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parse_config("config.toml", EXAMPLE)?;
        assert!(parsed.warnings.is_empty());
        assert_eq!(
            parsed
                .config
                .project
                .as_ref()
                .and_then(|project| project.default_module.as_deref()),
            Some(":app")
        );
        assert!(parsed.document.render().contains("# shared configuration"));
        Ok(())
    }

    #[test]
    fn warns_without_deleting_unknown_fields() -> Result<(), Box<dyn std::error::Error>> {
        let input = "schema_version = 1\n[ui]\nunknown_option = true\n";
        let parsed = parse_config("config.toml", input)?;
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.warnings[0].path, "ui.unknown_option");
        assert!(parsed.document.render().contains("unknown_option"));
        Ok(())
    }

    #[test]
    fn rejects_shell_strings_and_literal_secrets() {
        let shell = "schema_version = 1\n[commands.bad]\ncommand = \"echo unsafe\"\n";
        assert!(matches!(
            parse_config("config.toml", shell),
            Err(ConfigError::Parse { .. })
        ));

        let secret = "schema_version = 1\n[profiles.bad.environment]\nAPI_TOKEN = \"secret\"\n";
        assert!(matches!(
            parse_config("config.toml", secret),
            Err(ConfigError::ValidationAt { .. })
        ));
    }

    #[test]
    fn applies_layers_from_low_to_high() -> Result<(), Box<dyn std::error::Error>> {
        let shared = ConfigLayer {
            project: crate::ProjectConfig {
                default_module: Some(":shared".into()),
                default_variant: None,
            },
            gradle: crate::GradleConfig {
                arguments: Some(vec!["--stacktrace".into()]),
            },
            ..ConfigLayer::default()
        };
        let user = ConfigLayer {
            project: crate::ProjectConfig {
                default_module: None,
                default_variant: Some("demoDebug".into()),
            },
            ui: crate::UiConfig {
                keymap: Some(KeymapPreset::Vim),
                reduced_motion: None,
                unicode: Some(UnicodeMode::Ascii),
            },
            ..ConfigLayer::default()
        };
        let cli = ConfigLayer {
            project: crate::ProjectConfig {
                default_module: Some(":cli".into()),
                default_variant: None,
            },
            ..ConfigLayer::default()
        };

        let mut resolver = ConfigResolver::new();
        resolver
            .apply(&shared, "shared.toml")?
            .apply(&user, "user.toml")?
            .apply(&cli, "cli")?;
        let resolved = resolver.finish();

        assert_eq!(resolved.project.default_module.as_deref(), Some(":cli"));
        assert_eq!(
            resolved.project.default_variant.as_deref(),
            Some("demoDebug")
        );
        assert_eq!(resolved.gradle_arguments, ["--stacktrace"]);
        assert_eq!(resolved.keymap, KeymapPreset::Vim);
        assert_eq!(resolved.unicode, UnicodeMode::Ascii);
        Ok(())
    }

    #[test]
    fn local_migration_backs_up_and_shared_requires_confirmation()
    -> Result<(), Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("config.toml");
        fs::write(&path, EXAMPLE)?;
        let parsed = parse_config(&path, EXAMPLE)?;

        let backup = write_config_migration(&path, &parsed.document, ConfigScope::Local, false)?;
        assert!(backup.is_some_and(|backup| backup.exists()));

        assert!(matches!(
            write_config_migration(&path, &parsed.document, ConfigScope::Shared, false),
            Err(ConfigError::SharedMigrationConfirmationRequired { .. })
        ));
        Ok(())
    }

    #[test]
    fn rejects_invalid_log_buffer_and_environment_reference() {
        let buffer = "schema_version = 1\n[logcat]\nbuffer_mib = 7\n";
        assert!(matches!(
            parse_config("config.toml", buffer),
            Err(ConfigError::ValidationAt { .. })
        ));

        let mut environment = BTreeMap::new();
        environment.insert(
            "API_TOKEN".into(),
            EnvironmentValue::FromEnvironment {
                from_env: "not-valid-name!".into(),
            },
        );
        let layer = ConfigLayer {
            profiles: BTreeMap::from([(
                "bad".into(),
                crate::ProfileConfig {
                    environment,
                    ..crate::ProfileConfig::default()
                },
            )]),
            ..ConfigLayer::default()
        };
        assert!(layer.validate(PathBuf::from("config.toml")).is_err());
    }

    #[test]
    fn warns_for_large_but_valid_log_buffer() -> Result<(), Box<dyn std::error::Error>> {
        let input = "schema_version = 1\n[logcat]\nbuffer_mib = 512\n";
        let parsed = parse_config("config.toml", input)?;
        assert!(
            parsed
                .warnings
                .iter()
                .any(|warning| warning.path == "logcat.buffer_mib")
        );
        Ok(())
    }
}
