use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Diagnostic;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectModel {
    pub root: PathBuf,
    pub build: BuildInfo,
    pub included_builds: Vec<IncludedBuild>,
    pub modules: Vec<AndroidModule>,
    pub tasks: Vec<GradleTask>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
}

impl ProjectModel {
    #[must_use]
    pub fn empty(root: PathBuf) -> Self {
        Self {
            build: BuildInfo {
                root: root.clone(),
                gradle_version: String::new(),
                agp_version: None,
                java_version: None,
                kotlin_plugin_version: None,
            },
            root,
            included_builds: Vec::new(),
            modules: Vec::new(),
            tasks: Vec::new(),
            diagnostics: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildInfo {
    pub root: PathBuf,
    pub gradle_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agp_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub java_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kotlin_plugin_version: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IncludedBuild {
    pub id: String,
    pub root: PathBuf,
    pub primary: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ModuleKind {
    Application,
    Library,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AndroidModule {
    pub path: String,
    pub build_id: String,
    pub kind: ModuleKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compile_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_sdk: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_sdk: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flavor_dimensions: Vec<FlavorDimension>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub product_flavors: Vec<ProductFlavor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub build_types: Vec<BuildType>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FlavorDimension {
    pub name: String,
    pub order: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductFlavor {
    pub name: String,
    pub dimension: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuildType {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debuggable: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantFlavor {
    pub dimension: String,
    pub flavor: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Variant {
    pub name: String,
    pub enabled: bool,
    pub build_type: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flavors: Vec<VariantFlavor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub debuggable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub launcher: Option<LaunchComponent>,
    pub tasks: VariantTasks,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<Artifact>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_components: Vec<TestComponent>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantTasks {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assemble: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bundle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lint: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchComponent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    pub activity: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ArtifactKind {
    Apk,
    TestApk,
    Bundle,
    Aar,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub kind: ArtifactKind,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TestComponentKind {
    Unit,
    Instrumentation,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestComponent {
    pub name: String,
    pub kind: TestComponentKind,
    pub task: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runner: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub result_directories: Vec<PathBuf>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TaskKind {
    Assemble,
    Bundle,
    Install,
    UnitTest,
    InstrumentationTest,
    Lint,
    Verification,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GradleTask {
    pub path: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub origin_build: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    pub kind: TaskKind,
}
