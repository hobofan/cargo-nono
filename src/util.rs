use std::env;
use std::process::Command;
use std::str::from_utf8;
use cargo_metadata::{Dependency, Metadata, Package, WorkspaceMember};

use ext::{Feature, FeatureCause};

pub fn metadata_run(additional_args: Option<String>) -> Result<Metadata, ()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| String::from("cargo"));
    let mut cmd = Command::new(cargo);
    cmd.arg("metadata");
    cmd.args(&["--format-version", "1"]);
    if let Some(additional_args) = additional_args {
        cmd.arg(&additional_args);
    }

    let output = cmd.output().unwrap();
    let stdout = from_utf8(&output.stdout).unwrap();
    let meta = serde_json::from_str(stdout).unwrap();
    Ok(meta)
}

pub fn features_from_args(
    package_id: String,
    no_default: bool,
    features_args: Vec<String>,
) -> Vec<Feature> {
    let mut features = Vec::new();
    if !no_default {
        let mut feature = Feature::new(package_id.clone(), "default".to_owned());
        feature.causes.push(FeatureCause::Default);
        features.push(feature);
    }
    for features_args_str in features_args {
        let feats = features_args_str.split(",");
        for feat in feats {
            let mut feature = Feature::new(package_id.clone(), feat.to_owned());
            feature.causes.push(FeatureCause::CliFlag(feat.to_owned()));
            features.push(feature);
        }
    }

    features
}

pub fn main_ws_member_from_args<'a>(
    metadata: &'a Metadata,
    package_arg: Option<&str>,
) -> &'a WorkspaceMember {
    let target_workspace_member;
    if metadata.workspace_members.len() == 1 {
        target_workspace_member = metadata.workspace_members.get(0).unwrap();
    } else {
        let package_names: Vec<_> = metadata
            .workspace_members
            .iter()
            .map(|n| n.name())
            .collect();
        match package_arg {
            Some(package_name) => {
                let member = metadata
                    .workspace_members
                    .iter()
                    .find(|n| n.name() == package_name);
                if member.is_none() {
                    println!(
                        "Unknown package \"{}\". Please provide on of {:?} via --package flag.",
                        package_name, package_names
                    );
                    std::process::exit(1);
                }
                target_workspace_member = member.unwrap();
            }
            None => {
                println!("Multiple packages present in workspace. Please provide on of {:?} via --package flag.", package_names);
                std::process::exit(1);
            }
        }
    }
    target_workspace_member
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
