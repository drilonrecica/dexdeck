use std::path::PathBuf;

use dexdeck_protocol::{AndroidModule, BuildInfo, GradleTask, ProjectModel};

#[derive(Debug, Default)]
pub struct Agp8ModelBuilder {
    build: Option<BuildInfo>,
    modules: Vec<AndroidModule>,
    tasks: Vec<GradleTask>,
}

impl Agp8ModelBuilder {
    pub const MINIMUM: &'static str = "8.0.2";
    pub const TESTED_UPPER: &'static str = "8.13.0";

    pub fn build_info(&mut self, build: BuildInfo) {
        self.build = Some(build);
    }
    pub fn module(&mut self, module: AndroidModule) {
        self.modules.push(module);
    }
    pub fn task(&mut self, task: GradleTask) {
        self.tasks.push(task);
    }

    pub fn finish(mut self, root: PathBuf) -> ProjectModel {
        self.modules.sort_by(|a, b| a.path.cmp(&b.path));
        self.tasks.sort_by(|a, b| a.path.cmp(&b.path));
        ProjectModel {
            root: root.clone(),
            build: self.build.unwrap_or(BuildInfo {
                root,
                gradle_version: String::new(),
                agp_version: None,
                java_version: None,
                kotlin_plugin_version: None,
            }),
            included_builds: Vec::new(),
            modules: self.modules,
            tasks: self.tasks,
            diagnostics: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatibility_bounds_are_explicit() {
        assert_eq!(Agp8ModelBuilder::MINIMUM, "8.0.2");
        assert_eq!(Agp8ModelBuilder::TESTED_UPPER, "8.13.0");
    }
}
