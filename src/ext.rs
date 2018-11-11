use std::collections::HashSet;
use cargo_metadata::{Dependency, DependencyKind, Package};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Feature {
    pub inner: String,
}

impl Feature {
    pub fn new(feature: String) -> Self {
        Self { inner: feature }
    }
}

pub trait PackageExt {
    /// Receives a list of activated features (features activated by other features have already
    /// been calculate), and should return a list of dependencies that are activated for that
    /// featureset.
    fn active_dependencies(&self, features: &[Feature]) -> Vec<Dependency>;

    fn always_on_dependencies(&self) -> Vec<Dependency>;
    /// Active dependencies for a single feature.
    fn active_dependencies_for_feature(&self, feature: &Feature) -> Vec<Dependency>;
    /// Resolve all features that are activated by the provided feature. Also includes the
    /// provided feature.
    fn active_features_for_feature(&self, feature: &Feature) -> Vec<Feature>;
    fn active_features_for_features(&self, features: &[Feature]) -> Vec<Feature> {
        let mut resolved_features = HashSet::new();
        for feature in features {
            for resolved_feature in self.active_features_for_feature(feature) {
                resolved_features.insert(resolved_feature);
            }
        }
        resolved_features.into_iter().collect()
    }

    fn lib_target_sources(&self) -> Vec<String>;

    fn is_proc_macro(&self) -> bool;
}

impl PackageExt for Package {
    fn active_dependencies(&self, features: &[Feature]) -> Vec<Dependency> {
        let mut dependencies = Vec::new();
        for dep in self.always_on_dependencies() {
            dependencies.push(dep);
        }

        for feature in features {
            let deps = self.active_dependencies_for_feature(feature);
            for dep in deps.into_iter() {
                dependencies.push(dep);
            }
        }
        dependencies.dedup_by(|a, b| a.name == b.name);
        dependencies = dependencies
            .into_iter()
            .filter(|dep| dep.kind == DependencyKind::Normal)
            .collect();

        dependencies
    }

    fn active_dependencies_for_feature(&self, feature: &Feature) -> Vec<Dependency> {
        let activated_features: Vec<Feature> = self.features
            .get(&feature.inner)
            .map(|features| features.clone().into_iter().map(Feature::new).collect())
            .unwrap_or_default();

        self.dependencies
            .iter()
            .filter(|dependency| {
                for feature in activated_features.iter() {
                    if feature.inner == dependency.name {
                        return true;
                    }
                }
                return false;
            })
            .map(|n| n.to_owned())
            .collect()
    }

    fn active_features_for_feature(&self, feature: &Feature) -> Vec<Feature> {
        let mut resolved_features: HashSet<Feature> = HashSet::new();
        let mut unresolved_features: HashSet<Feature> = HashSet::new();
        unresolved_features.insert(feature.to_owned());

        while !unresolved_features.is_empty() {
            for unresolved in unresolved_features.clone().iter() {
                let activated_features: Vec<Feature> = self.features
                    .get(&unresolved.inner)
                    .map(|features| features.clone().into_iter().map(Feature::new).collect())
                    .unwrap_or_default();
                unresolved_features.remove(&unresolved);
                resolved_features.insert(unresolved.to_owned());
                for activated in activated_features {
                    if !resolved_features.contains(&activated) {
                        unresolved_features.insert(activated);
                    }
                }
            }
        }

        resolved_features.into_iter().collect()
    }

    fn always_on_dependencies(&self) -> Vec<Dependency> {
        self.dependencies
            .iter()
            .filter(|dep| !dep.optional)
            .map(|n| n.to_owned())
            .collect()
    }

    fn lib_target_sources(&self) -> Vec<String> {
        self.targets
            .iter()
            .filter(|target| target.kind.contains(&"lib".to_string()))
            .map(|target| target.src_path.clone())
            .collect()
    }

    fn is_proc_macro(&self) -> bool {
        self.targets
            .iter()
            .filter(|target| target.kind.contains(&"proc-macro".to_string()))
            .next()
            .is_some()
    }
}
