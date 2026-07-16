pub use dexdeck_protocol::ModelFreshness as Freshness;
use dexdeck_protocol::ProjectModel;

#[derive(Clone, Debug)]
pub struct ModelRefresh {
    model: Option<ProjectModel>,
    freshness: Freshness,
    generation: u64,
}

impl ModelRefresh {
    #[must_use]
    pub fn new(model: Option<ProjectModel>) -> Self {
        Self {
            freshness: if model.is_some() {
                Freshness::Stale
            } else {
                Freshness::Provisional
            },
            model,
            generation: 0,
        }
    }
    #[must_use]
    pub fn model(&self) -> Option<&ProjectModel> {
        self.model.as_ref()
    }
    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }
    pub fn begin(&mut self) -> u64 {
        self.generation += 1;
        self.freshness = Freshness::Refreshing;
        self.generation
    }
    pub fn complete(&mut self, generation: u64, model: ProjectModel) -> bool {
        if generation != self.generation {
            return false;
        }
        self.model = Some(model);
        self.freshness = Freshness::Current;
        true
    }
    pub fn fail(&mut self, generation: u64, degraded: bool) {
        if generation == self.generation {
            self.freshness = if degraded {
                Freshness::Degraded
            } else {
                Freshness::Stale
            };
        }
    }
    pub fn cancel(&mut self) {
        self.generation += 1;
        self.freshness = if self.model.is_some() {
            Freshness::Stale
        } else {
            Freshness::Provisional
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    #[test]
    fn failure_preserves_snapshot() {
        let model = ProjectModel::empty(PathBuf::from("root"));
        let mut state = ModelRefresh::new(Some(model.clone()));
        let id = state.begin();
        state.fail(id, false);
        assert_eq!(state.model(), Some(&model));
    }
    #[test]
    fn cancellation_rejects_late_result() {
        let mut state = ModelRefresh::new(None);
        let id = state.begin();
        state.cancel();
        assert!(!state.complete(id, ProjectModel::empty(PathBuf::from("root"))));
    }
}
