mod check;
mod check_source;
mod ext;
mod util;

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
                    println!("  - Did not find a #![no_std] attribute or a simple conditional attribute like #![cfg_attr(not(feature = \"std\"), no_std)] in the crate source. Crate most likely doesn't support no_std without changes.");
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
        );

    let matches = app.clone().get_matches();
    if let Some(matches) = matches.subcommand_matches("check") {
        let metadata_full = metadata_run(Some("--all-features".to_owned())).unwrap();
        let metadata = metadata_run(None).unwrap();

        let target_workspace_member =
            main_ws_member_from_args(&metadata, matches.value_of("package"));

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
    app.print_help().unwrap();
    println!(""); // print newline since print_help doesn't do that
}
