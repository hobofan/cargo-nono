use std::env;
use std::process::Command;
use std::str::from_utf8;
use cargo_metadata::{Dependency, Metadata, Package};

use ext::Feature;

pub fn metadata_run(additional_args: Option<String>) -> Result<Metadata, ()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    let mut cmd = Command::new(cargo);
    cmd.arg("metadata");
    cmd.args(&["--format-version", "1"]);
    cmd.arg("--all-features");
    if let Some(additional_args) = additional_args {
        cmd.arg(&additional_args);
    }

    let output = cmd.output().unwrap();
    let stdout = from_utf8(&output.stdout).unwrap();
    let meta = serde_json::from_str(stdout).unwrap();
    Ok(meta)
}

pub fn features_from_args(no_default: bool, features_args: Vec<String>) -> Vec<Feature> {
    let mut features = Vec::new();
    if !no_default {
        features.push(Feature::new("default".to_owned()));
    }
    for features_args_str in features_args {
        let feats = features_args_str.split(",");
        for feat in feats {
            features.push(Feature::new(feat.to_owned()));
        }
    }

    features
}

pub fn dependencies_to_packages(
    package: &Package,
    metadata: &Metadata,
    dependencies: &[Dependency],
) -> Vec<Package> {
    let resolve_node = metadata
        .resolve
        .clone()
        .unwrap()
        .nodes
        .into_iter()
        .find(|n| n.id == package.id)
        .unwrap();
    // All dependency packages of the package
    let dependency_packages: Vec<Package> = metadata
        .packages
        .iter()
        .filter(|n| resolve_node.dependencies.contains(&n.id))
        .map(|n| n.clone())
        .collect();

    // limit packages to only the activated dependencies
    dependency_packages
        .into_iter()
        .filter(|package| {
            for dependency in dependencies.iter() {
                if package.name == dependency.name {
                    return true;
                }
            }
            return false;
        })
        .collect()
}
