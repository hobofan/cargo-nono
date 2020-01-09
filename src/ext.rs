use cargo_metadata::{Dependency, DependencyKind, Metadata, Package};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Feature {
    pub package_id: String,
    pub name: String,
    pub causes: Vec<FeatureCause>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FeatureCause {
    /// Feature is triggered by another feature.
    // (package_id, feature_name)
    Feature(Box<Feature>),
    /// Feature is activated by default in its respective package.
    // (package_id)
    Default(String),
    /// Feature is activated by explicityly by the provided package_id.
    // (package_id)
    Explicit(String),
    /// Feature has been activated via a --features flag.
    CliFlag(String),
    // Unknown,
}

impl Feature {
    pub fn new(package_id: String, feature: String) -> Self {
        Self {
            package_id,
            name: feature,
            causes: Vec::new(),
        }
    }

    pub fn print(&self, metadata: &Metadata, offset: usize) {
        let package_print_name = |package_id| {
            let package = metadata.find_package(package_id);
            if package.is_none() {
                return "UNPRINTABLE".to_owned();
            }
            let package = package.unwrap();
            format!("{}:{}", package.name, package.version)
        };
        for _ in 0..offset {
            print!("  ");
        }
        println!(
            "- Caused by feature flag \"{}\" in crate \"{}\"",
            self.name,
            package_print_name(&self.package_id)
        );
        for cause in self.causes.iter() {
            cause.print(metadata, offset + 1);
        }
    }
}

impl FeatureCause {
    pub fn print(&self, metadata: &Metadata, offset: usize) {
        let print_offset = || {
            for _ in 0..offset {
                print!("  ");
            }
        };
        let package_print_name = |package_id| {
            let package = metadata.find_package(package_id);
            if package.is_none() {
                return "UNPRINTABLE".to_owned();
            }
            let package = package.unwrap();
            format!("{}:{}", package.name, package.version)
        };
        match self {
            FeatureCause::Feature(feat) => feat.print(metadata, offset),
            FeatureCause::CliFlag(flag) => {
                print_offset();
                println!("- Caused by providing CLI --features flag \"{}\"", flag)
            }
            FeatureCause::Default(package_id) => {
                print_offset();
                println!(
                    "- Caused by implicitly enabled default feature from \"{}\"",
                    package_print_name(package_id)
                )
            }
            FeatureCause::Explicit(package_id) => {
                print_offset();
                println!(
                    "- Explicityly enabled feature from \"{}\"",
                    package_print_name(package_id)
                )
            }
        }
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

    /// Tries to turn a feature like "serde/std" into a feature flag on "serde".
    fn dependency_feature_for_feature(
        &self,
        metadata: &Metadata,
        feature: &Feature,
    ) -> Option<Feature>;

    fn dependency_features_for_features(
        &self,
        metadata: &Metadata,
        features: &[Feature],
    ) -> Vec<Feature> {
        features
            .iter()
            .filter_map(|feature| self.dependency_feature_for_feature(metadata, feature))
            .collect()
    }

    fn all_dependency_features(
        &self,
        metadata: &Metadata,
        external_features: &[Feature],
    ) -> Vec<Feature> {
        let mut features = self.fixed_dependency_features(metadata);
        for feat in self.dependency_features_for_features(metadata, external_features) {
            features.push(feat);
        }

        features
    }

    /// Fixed dependency features. Those are hardcoded in the Cargo.toml of the package and can not
    /// be deactivated by turning off its default features.
    fn fixed_dependency_features(&self, metadata: &Metadata) -> Vec<Feature>;

    fn lib_target_sources(&self) -> Vec<String>;
    fn bin_target_sources(&self) -> Vec<String>;

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
        let activated_features = self.active_features_for_feature(feature);

        self.dependencies
            .iter()
            .filter(|dependency| {
                for feature in activated_features.iter() {
                    if feature.name == dependency.name {
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
                let activated_features: Vec<Feature> = self
                    .features
                    .get(&unresolved.name)
                    .map(|features| {
                        features
                            .clone()
                            .into_iter()
                            .map(|raw_feature| {
                                let mut new_feature =
                                    Feature::new(self.id.repr.clone(), raw_feature);
                                new_feature
                                    .causes
                                    .push(FeatureCause::Feature(Box::new(feature.clone())));

                                new_feature
                            })
                            .collect()
                    })
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

    fn dependency_feature_for_feature(
        &self,
        metadata: &Metadata,
        feature: &Feature,
    ) -> Option<Feature> {
        if !feature.name.contains("/") {
            return None;
        }

        let dependency_feature_parts: Vec<_> = feature.name.split("/").collect();
        let dependency_name = dependency_feature_parts[0];
        let dependency_feature_name = dependency_feature_parts[1];
        let dependency = self.dependencies.iter().find(|n| n.name == dependency_name);
        if dependency.is_none() {
            return None;
        }

        let dep_package_id = metadata.dependency_package_id(self, dependency.unwrap());
        // package_id of dependency might not be findable if we try to activate the feature of a
        // optional dependency
        if dep_package_id.is_none() {
            return None;
        }

        let mut new_feature =
            Feature::new(dep_package_id.unwrap(), dependency_feature_name.to_owned());
        new_feature
            .causes
            .push(FeatureCause::Feature(Box::new(feature.clone())));

        Some(new_feature)
    }

    fn fixed_dependency_features(&self, metadata: &Metadata) -> Vec<Feature> {
        self.dependencies
            .iter()
            .flat_map(|dependency| {
                let dep_package_id = metadata.dependency_package_id(self, dependency);
                // package_id of dependency might not be findable if we try to activate the feature of a
                // optional dependency
                if dep_package_id.is_none() {
                    return Vec::new();
                }
                let dep_package_id = dep_package_id.unwrap();
                // features activated via
                // serde = { version = "*", features = ["std"] }
                //                                      ^^^^^
                let mut explicit_dependency_features = dependency
                    .features
                    .clone()
                    .into_iter()
                    .map(|raw_feature| {
                        let mut feature = Feature::new(dep_package_id.to_owned(), raw_feature);
                        feature
                            .causes
                            .push(FeatureCause::Explicit(self.id.repr.clone()));
                        feature
                    })
                    .collect::<Vec<_>>();
                // features activated via
                // serde = { version = "*", default-features = true }
                //                                             ^^^^
                // or the absence of the default-features option
                if dependency.uses_default_features {
                    let mut feature = Feature::new(dep_package_id.to_owned(), "default".to_owned());
                    feature
                        .causes
                        .push(FeatureCause::Default(self.id.repr.clone()));

                    explicit_dependency_features.push(feature);
                }
                explicit_dependency_features
            })
            .collect()
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
            .flat_map(|target| target.src_path.to_str())
            .map(|target| target.into())
            .collect()
    }

    fn bin_target_sources(&self) -> Vec<String> {
        self.targets
            .iter()
            .filter(|target| target.kind.contains(&"bin".to_string()))
            .flat_map(|target| target.src_path.to_str())
            .map(|target| target.into())
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

pub trait MetadataExt {
    fn find_package(&self, package_id: &str) -> Option<&Package>;
    fn dependency_package_id(&self, package: &Package, dependency: &Dependency) -> Option<String>;
}

impl MetadataExt for Metadata {
    fn find_package(&self, package_id: &str) -> Option<&Package> {
        self.packages
            .iter()
            .find(|package| package.id.repr == package_id)
    }

    fn dependency_package_id(&self, package: &Package, dependency: &Dependency) -> Option<String> {
        let resolve_node = self
            .resolve
            .clone()
            .unwrap()
            .nodes
            .into_iter()
            .find(|n| n.id == package.id)
            .unwrap();
        // All dependency packages of the package
        let dependency_packages: Vec<Package> = self
            .packages
            .iter()
            .filter(|n| resolve_node.dependencies.contains(&n.id))
            .map(|n| n.clone())
            .collect();

        dependency_packages
            .into_iter()
            .find(|package| package.name == dependency.name)
            .map(|n| n.id.repr)
    }
}

pub trait EscargotBuildExt {
    fn set_features(self, no_default: bool, features_args: Vec<String>) -> Self;
}

impl EscargotBuildExt for escargot::CargoBuild {
    fn set_features(self, no_default: bool, features_args: Vec<String>) -> Self {
        let mut build = self;
        if no_default {
            build = build.no_default_features();
        }
        build.features(features_args.join(" "))
    }
}
