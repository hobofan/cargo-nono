mod check;
mod check_source;
mod ext;
mod util;
mod verify;

use clap::{App, Arg, SubCommand};
use console::Emoji;
use std::path::PathBuf;

use crate::check::*;
use crate::check_source::*;
use crate::ext::*;
use crate::util::*;

use cargo_metadata::{Metadata, Package};

pub static SUCCESS: Emoji = Emoji("✅  ", "SUCCESS");
pub static FAILURE: Emoji = Emoji("❌  ", "FAILURE");
pub static MAYBE: Emoji = Emoji("❓  ", "MAYBE");

fn check_and_print_package(
    package: &Package,
    resolved_dependency_features: &[Feature],
    metadata: &Metadata,
    metadata_full: &Metadata,
    is_main_pkg: bool,
) -> bool {
    let mut package_did_fail = false;

    let package_features: Vec<Feature> = resolved_dependency_features
        .iter()
        .filter(|n| n.package_id == package.id.repr)
        .map(|n| n.to_owned())
        .collect();
    let active_features = package.active_features_for_features(&package_features);
    let active_dependencies = package.active_dependencies(&active_features);
    let _active_packages = dependencies_to_packages(&package, &metadata_full, &active_dependencies);
    let _resolved_dependency_features =
        package.all_dependency_features(&metadata_full, &active_features);

    let mut support = CrateSupport::NoOffenseDetected;
    if package.is_proc_macro() {
        support = CrateSupport::ProcMacro;
    }
    if support == CrateSupport::NoOffenseDetected {
        match is_main_pkg {
            false => {
                let srcs: Vec<_> = package
                    .lib_target_sources()
                    .into_iter()
                    .map(PathBuf::from)
                    .collect();
                // TODO: check more than one
                support = srcs
                    .into_iter()
                    .map(|src_path| get_crate_support_from_source(&src_path))
                    .next()
                    .unwrap_or(CrateSupport::NoOffenseDetected);
            }
            true => {
                let srcs: Vec<_> = package
                    .bin_target_sources()
                    .into_iter()
                    .chain(package.lib_target_sources())
                    .map(PathBuf::from)
                    .collect();
                support = srcs
                    .into_iter()
                    .map(|src_path| get_crate_support_from_source(&src_path))
                    .next()
                    .unwrap_or(CrateSupport::NoOffenseDetected);
            }
        }
    }

    let check = CheckResult {
        package_name: package.name.clone(),
        support,
        active_features: active_features,
    };

    // set flag that at least one crate check failed
    if !check.no_std_itself() {
        package_did_fail = true;
    }
    let overall_res = match check.no_std_itself() {
        true => SUCCESS,
        false => FAILURE,
    };
    println!("{}: {}", check.package_name, overall_res);
    if check.no_std_itself() {
        return package_did_fail;
    }
    if let CrateSupport::OnlyWithoutFeature(feature) = &check.support {
        println!(
            "  - Crate supports no_std if \"{}\" feature is deactivated.",
            feature
        );
        let feat = check.find_active_feature_by_name(&feature).unwrap();
        feat.print(&metadata, 2);
    }
    if let CrateSupport::SourceOffenses(ref offenses) = check.support {
        for offense in offenses {
            match offense {
                SourceOffense::MissingNoStdAttribute => {
                    println!("  - Did not find a #![no_std] attribute or a simple conditional attribute like #[cfg_attr(not(feature = \"std\"), no_std)] in the crate source. Crate most likely doesn't support no_std without changes.");
                }
                SourceOffense::UseStdStatement(stmt) => {
                    println!("  - Source code contains an explicit `use std::` statement.");
                    println!("{}", stmt);
                }
            }
        }
    }

    package_did_fail
}

fn run_check(matches: &clap::ArgMatches) {
    let metadata_full = metadata_run(Some("--all-features".to_owned())).unwrap();
    let metadata = metadata_run(None).unwrap();

    let target_workspace_member = main_ws_member_from_args(&metadata, matches.value_of("package"));

    let target_package = metadata
        .find_package(&target_workspace_member.repr)
        .unwrap();
    let features = features_from_args(
        target_package.id.repr.clone(),
        matches.is_present("no-default-features"),
        matches
            .values_of("features")
            .map(|n| n.into_iter().map(|m| m.to_owned()).collect())
            .unwrap_or(Vec::new())
            .to_owned(),
    );

    let active_features = target_package.active_features_for_features(&features);
    let active_dependencies = target_package.active_dependencies(&active_features);
    let active_packages =
        dependencies_to_packages(&target_package, &metadata_full, &active_dependencies);

    let mut package_did_fail = false;
    let resolved_dependency_features =
        target_package.all_dependency_features(&metadata_full, &active_features);

    let main_package = metadata
        .packages
        .iter()
        .find(|n| &n.id == target_workspace_member)
        .expect("Unable to find main package.");
    if check_and_print_package(
        main_package,
        &resolved_dependency_features,
        &metadata,
        &metadata_full,
        true,
    ) {
        package_did_fail = true;
    }

    for package in active_packages.iter() {
        if check_and_print_package(
            package,
            &resolved_dependency_features,
            &metadata,
            &metadata_full,
            false,
        ) {
            package_did_fail = true;
        }
    }
    match package_did_fail {
        true => std::process::exit(1),
        false => std::process::exit(0),
    }
}

fn active_packages(matches: &clap::ArgMatches) -> Vec<Package> {
    let metadata_full = metadata_run(Some("--all-features".to_owned())).unwrap();
    let metadata = metadata_run(None).unwrap();

    let target_workspace_member = main_ws_member_from_args(&metadata, matches.value_of("package"));

    let target_package = metadata
        .find_package(&target_workspace_member.repr)
        .unwrap();
    let features = features_from_args(
        target_package.id.repr.clone(),
        matches.is_present("no-default-features"),
        matches
            .values_of("features")
            .map(|n| n.into_iter().map(|m| m.to_owned()).collect())
            .unwrap_or(Vec::new())
            .to_owned(),
    );

    let active_features = target_package.active_features_for_features(&features);
    let active_dependencies = target_package.active_dependencies(&active_features);
    let active_packages =
        dependencies_to_packages(&target_package, &metadata_full, &active_dependencies);

    active_packages
}

fn run_verify(matches: &clap::ArgMatches) {
    // First run a normal build so we see build progress
    let mut build_args = vec!["build"];
    if matches.is_present("no-default-features") {
        build_args.push("--no-default-features");
    }
    let features_arg = matches
        .values_of("features")
        .map(|n| n.into_iter().map(|m| m.to_owned()).collect())
        .unwrap_or(Vec::new())
        .to_owned()
        .join(",");
    if !features_arg.is_empty() {
        build_args.push("--features");
        build_args.push(&features_arg);
    }
    duct::cmd("cargo", &build_args).run().unwrap();

    let build_result = escargot::CargoBuild::new()
        .set_features(
            matches.is_present("no-default-features"),
            matches
                .values_of("features")
                .map(|n| n.into_iter().map(|m| m.to_owned()).collect())
                .unwrap_or(Vec::new())
                .to_owned(),
        )
        .exec()
        .unwrap();
    let raw_messages: Vec<escargot::Message> = build_result
        .into_iter()
        .filter_map(|raw_msg| raw_msg.ok())
        .collect::<Vec<_>>();
    let decoded_messages = raw_messages
        .iter()
        .filter_map(|raw_msg| raw_msg.decode().ok())
        .collect::<Vec<_>>();

    let as_compiler_artifact = |msg| {
        if let escargot::format::Message::CompilerArtifact(artifact) = msg {
            return Some(artifact);
        }
        None
    };
    let artifact_filenames_for_message = |msg| {
        as_compiler_artifact(msg).map(|artifact| {
            let artifact_filenames = artifact
                .filenames
                .into_iter()
                .map(|n| n.into_owned())
                .collect::<Vec<_>>();
            artifact_filenames
        })
    };

    let artifact_messages = decoded_messages
        .clone()
        .into_iter()
        .filter_map(artifact_filenames_for_message)
        .collect::<Vec<_>>();

    let main_artifact_message = artifact_messages.last().unwrap();
    let main_artifact_path = main_artifact_message.first().unwrap();

    let main_has_std = verify::rlib_contains_namespace(&main_artifact_path, "std");
    let active_packages: Vec<String> = active_packages(matches)
        .into_iter()
        .map(|pkg| pkg.name.to_owned())
        .collect();
    for (i, msg) in decoded_messages.clone().into_iter().enumerate() {
        let is_last = i == decoded_messages.len() - 1;
        let artifact_filenames = artifact_filenames_for_message(msg.clone());
        if let Some(filenames) = artifact_filenames {
            let dependency_name = as_compiler_artifact(msg).unwrap().target.name;
            if !active_packages.contains(&dependency_name.to_string()) && !is_last {
                continue;
            }
            let artifact_path = filenames.first().unwrap();
            if artifact_path.extension() != Some(std::ffi::OsStr::new("rlib")) {
                continue;
            }

            let has_std = verify::rlib_contains_namespace(&artifact_path, "std");
            let icon = match has_std {
                true => FAILURE,
                false => SUCCESS,
            };
            println!("{} {}", icon, dependency_name);
        }
    }

    match main_has_std {
        true => std::process::exit(1),
        false => std::process::exit(0),
    }
}

fn main() {
    let mut app = App::new("cargo nono")
        .arg(Arg::with_name("dummy").hidden(true).possible_value("nono"))
        .subcommand(
            SubCommand::with_name("check")
                .arg(Arg::with_name("no-default-features").long("no-default-features"))
                .arg(
                    Arg::with_name("features")
                        .long("features")
                        .multiple(true)
                        .takes_value(true),
                )
                .arg(Arg::with_name("package").long("package").takes_value(true)),
        )
        .subcommand(
            SubCommand::with_name("verify")
                .arg(Arg::with_name("no-default-features").long("no-default-features"))
                .arg(
                    Arg::with_name("features")
                        .long("features")
                        .multiple(true)
                        .takes_value(true),
                )
                .arg(Arg::with_name("package").long("package").takes_value(true)),
        );

    let matches = app.clone().get_matches();
    if let Some(matches) = matches.subcommand_matches("check") {
        run_check(matches);
    }
    if let Some(matches) = matches.subcommand_matches("verify") {
        run_verify(matches);
    }
    app.print_help().unwrap();
    println!(""); // print newline since print_help doesn't do that
}
