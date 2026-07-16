use std::collections::{BTreeMap, BTreeSet};

use dexdeck_protocol::{
    AndroidModule, BridgeEnvelope, BridgePayload, BridgeProtocolError, BridgeStreamValidator,
    BuildInfo, Diagnostic, GradleTask, IncludedBuild, ProjectModel,
};
use sha2::{Digest, Sha256};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModelAssemblyError {
    #[error(transparent)]
    Protocol(#[from] BridgeProtocolError),
    #[error("bridge reported {code}: {message}")]
    Bridge { code: String, message: String },
    #[error("bridge build record must be the first model record")]
    BuildNotFirst,
    #[error("bridge emitted duplicate build record")]
    DuplicateBuild,
    #[error("bridge emitted duplicate included build {0}")]
    DuplicateIncludedBuild(String),
    #[error("bridge emitted duplicate module {0}")]
    DuplicateModule(String),
    #[error("bridge record references unknown module {0}")]
    UnknownModule(String),
    #[error("bridge emitted duplicate {kind} {name} for module {module}")]
    DuplicateComponent {
        module: String,
        kind: &'static str,
        name: String,
    },
    #[error("bridge emitted duplicate task {0}")]
    DuplicateTask(String),
    #[error("bridge stream contains no build record")]
    MissingBuild,
    #[error("bridge model hash mismatch: expected {expected}, calculated {actual}")]
    ModelHashMismatch { expected: String, actual: String },
    #[error("failed to serialize the normalized bridge model: {0}")]
    Serialization(String),
}

#[derive(Debug, Default)]
pub struct ModelAssembler {
    validator: BridgeStreamValidator,
    build: Option<BuildInfo>,
    included_builds: BTreeMap<String, IncludedBuild>,
    modules: BTreeMap<String, AndroidModule>,
    tasks: BTreeMap<String, GradleTask>,
    diagnostics: Vec<Diagnostic>,
    component_keys: BTreeSet<(String, &'static str, String)>,
}

impl ModelAssembler {
    pub fn accept(&mut self, record: BridgeEnvelope) -> Result<(), ModelAssemblyError> {
        self.validator.accept(&record)?;
        match record.payload {
            BridgePayload::Build { build } => {
                if self.build.is_some() {
                    return Err(ModelAssemblyError::DuplicateBuild);
                }
                if !self.modules.is_empty()
                    || !self.tasks.is_empty()
                    || !self.included_builds.is_empty()
                {
                    return Err(ModelAssemblyError::BuildNotFirst);
                }
                self.build = Some(build);
            }
            BridgePayload::IncludedBuild { included_build } => {
                self.require_build()?;
                let id = included_build.id.clone();
                if self
                    .included_builds
                    .insert(id.clone(), included_build)
                    .is_some()
                {
                    return Err(ModelAssemblyError::DuplicateIncludedBuild(id));
                }
            }
            BridgePayload::Module { module } => {
                self.require_build()?;
                let path = module.path.clone();
                if self.modules.contains_key(&path) {
                    return Err(ModelAssemblyError::DuplicateModule(path));
                }
                for (kind, name) in module
                    .flavor_dimensions
                    .iter()
                    .map(|value| ("dimension", value.name.as_str()))
                    .chain(
                        module
                            .product_flavors
                            .iter()
                            .map(|value| ("flavor", value.name.as_str())),
                    )
                    .chain(
                        module
                            .build_types
                            .iter()
                            .map(|value| ("buildType", value.name.as_str())),
                    )
                    .chain(
                        module
                            .variants
                            .iter()
                            .map(|value| ("variant", value.name.as_str())),
                    )
                {
                    let key = (path.clone(), kind, name.to_owned());
                    if !self.component_keys.insert(key) {
                        return Err(ModelAssemblyError::DuplicateComponent {
                            module: path,
                            kind,
                            name: name.to_owned(),
                        });
                    }
                }
                self.modules.insert(path, module);
            }
            BridgePayload::Dimension { module, dimension } => {
                self.component(&module, "dimension", &dimension.name)?;
                self.module_mut(&module)?.flavor_dimensions.push(dimension);
            }
            BridgePayload::Flavor { module, flavor } => {
                self.component(&module, "flavor", &flavor.name)?;
                self.module_mut(&module)?.product_flavors.push(flavor);
            }
            BridgePayload::Variant { module, variant } => {
                self.component(&module, "variant", &variant.name)?;
                self.module_mut(&module)?.variants.push(variant);
            }
            BridgePayload::Task { task } => {
                self.require_build()?;
                let path = task.path.clone();
                if self.tasks.insert(path.clone(), task).is_some() {
                    return Err(ModelAssemblyError::DuplicateTask(path));
                }
            }
            BridgePayload::Diagnostic { diagnostic } => self.diagnostics.push(diagnostic),
            BridgePayload::Error { code, message, .. } => {
                return Err(ModelAssemblyError::Bridge { code, message });
            }
            BridgePayload::Complete { .. } => {}
        }
        Ok(())
    }

    pub fn accept_json_line(&mut self, line: &str) -> Result<(), ModelAssemblyError> {
        let record = serde_json::from_str::<BridgeEnvelope>(line)
            .map_err(|error| BridgeProtocolError::InvalidJson(error.to_string()))?;
        self.accept(record)
    }

    pub fn finish(mut self) -> Result<ProjectModel, ModelAssemblyError> {
        let completion = self.validator.finish()?;
        let build = self.build.take().ok_or(ModelAssemblyError::MissingBuild)?;
        for module in self.modules.values_mut() {
            module
                .flavor_dimensions
                .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.name.cmp(&b.name)));
            module.product_flavors.sort_by(|a, b| {
                a.dimension
                    .cmp(&b.dimension)
                    .then_with(|| a.name.cmp(&b.name))
            });
            module.build_types.sort_by(|a, b| a.name.cmp(&b.name));
            module.variants.sort_by(|a, b| a.name.cmp(&b.name));
            for variant in &mut module.variants {
                variant.flavors.sort_by(|a, b| {
                    a.dimension
                        .cmp(&b.dimension)
                        .then_with(|| a.flavor.cmp(&b.flavor))
                });
                variant.artifacts.sort_by(|a, b| a.path.cmp(&b.path));
                variant.test_components.sort_by(|a, b| a.name.cmp(&b.name));
            }
        }
        self.diagnostics.sort_by(|a, b| a.message.cmp(&b.message));
        let model = ProjectModel {
            root: build.root.clone(),
            build,
            included_builds: self.included_builds.into_values().collect(),
            modules: self.modules.into_values().collect(),
            tasks: self.tasks.into_values().collect(),
            diagnostics: self.diagnostics,
        };
        let actual = model_hash(&model)?;
        if completion.model_hash != actual {
            return Err(ModelAssemblyError::ModelHashMismatch {
                expected: completion.model_hash,
                actual,
            });
        }
        Ok(model)
    }

    fn require_build(&self) -> Result<(), ModelAssemblyError> {
        self.build
            .as_ref()
            .map(|_| ())
            .ok_or(ModelAssemblyError::BuildNotFirst)
    }

    fn module_mut(&mut self, module: &str) -> Result<&mut AndroidModule, ModelAssemblyError> {
        self.modules
            .get_mut(module)
            .ok_or_else(|| ModelAssemblyError::UnknownModule(module.into()))
    }

    fn component(
        &mut self,
        module: &str,
        kind: &'static str,
        name: &str,
    ) -> Result<(), ModelAssemblyError> {
        if !self.modules.contains_key(module) {
            return Err(ModelAssemblyError::UnknownModule(module.into()));
        }
        let key = (module.to_owned(), kind, name.to_owned());
        if !self.component_keys.insert(key) {
            return Err(ModelAssemblyError::DuplicateComponent {
                module: module.into(),
                kind,
                name: name.into(),
            });
        }
        Ok(())
    }
}

pub fn model_hash(model: &ProjectModel) -> Result<String, ModelAssemblyError> {
    let bytes = serde_json::to_vec(model)
        .map_err(|error| ModelAssemblyError::Serialization(error.to_string()))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dexdeck_protocol::{BridgeComplete, ModuleKind};
    use std::path::PathBuf;

    fn build() -> BuildInfo {
        BuildInfo {
            root: PathBuf::from("/project"),
            gradle_version: "9.5".into(),
            agp_version: Some("9.3.0".into()),
            java_version: Some("17".into()),
            kotlin_plugin_version: None,
        }
    }
    fn module() -> AndroidModule {
        AndroidModule {
            path: ":app".into(),
            build_id: "root".into(),
            kind: ModuleKind::Application,
            namespace: None,
            compile_sdk: None,
            target_sdk: None,
            minimum_sdk: None,
            flavor_dimensions: Vec::new(),
            product_flavors: Vec::new(),
            build_types: Vec::new(),
            variants: Vec::new(),
        }
    }

    #[test]
    fn assembles_only_complete_hash_verified_models() -> Result<(), ModelAssemblyError> {
        let expected = ProjectModel {
            root: PathBuf::from("/project"),
            build: build(),
            included_builds: vec![],
            modules: vec![module()],
            tasks: vec![],
            diagnostics: vec![],
        };
        let mut assembler = ModelAssembler::default();
        assembler.accept(BridgeEnvelope::new(BridgePayload::Build { build: build() }))?;
        assembler.accept(BridgeEnvelope::new(BridgePayload::Module {
            module: module(),
        }))?;
        assembler.accept(BridgeEnvelope::new(BridgePayload::Complete {
            complete: BridgeComplete {
                duration_ms: 1,
                record_count: 2,
                model_hash: model_hash(&expected)?,
            },
        }))?;
        assert_eq!(assembler.finish()?, expected);
        Ok(())
    }

    #[test]
    fn rejects_error_records_without_partial_model() {
        let mut assembler = ModelAssembler::default();
        assembler
            .accept(BridgeEnvelope::new(BridgePayload::Build { build: build() }))
            .unwrap_or_else(|e| panic!("{e}"));
        assert!(matches!(
            assembler.accept(BridgeEnvelope::new(BridgePayload::Error {
                code: "adapter_unavailable".into(),
                message: "no adapter".into(),
                suggested_action: None
            })),
            Err(ModelAssemblyError::Bridge { .. })
        ));
    }
}
