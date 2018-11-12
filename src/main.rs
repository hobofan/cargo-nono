extern crate cargo_metadata;
extern crate clap;
extern crate console;
extern crate quote;
extern crate serde;
extern crate serde_json;
extern crate syn;

mod check;
mod ext;
mod util;

use std::path::PathBuf;
use console::Emoji;
use clap::{App, Arg, SubCommand};

use ext::*;
use util::*;
use check::*;

pub static SUCCESS: Emoji = Emoji("✅  ", "");
pub static FAILURE: Emoji = Emoji("❌  ", "");
pub static MAYBE: Emoji = Emoji("❓  ", "");

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

        let target_package = metadata.find_package(&target_workspace_member.raw).unwrap();
        let features = features_from_args(
            target_package.id.clone(),
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

        let resolved_dependency_features =
            target_package.all_dependency_features(&metadata_full, &active_features);
        for package in active_packages.iter() {
            let package_features: Vec<Feature> = resolved_dependency_features
                .iter()
                .filter(|n| n.package_id == package.id)
                .map(|n| n.to_owned())
                .collect();
            let active_features = package.active_features_for_features(&package_features);
            let active_dependencies = package.active_dependencies(&active_features);
            let _active_packages =
                dependencies_to_packages(&package, &metadata_full, &active_dependencies);
            let _resolved_dependency_features =
                package.all_dependency_features(&metadata_full, &active_features);

            let srcs: Vec<_> = package
                .lib_target_sources()
                .into_iter()
                .map(PathBuf::from)
                .collect();
            let mut support = CrateSupport::NotDetected;
            if package.is_proc_macro() {
                support = CrateSupport::ProcMacro;
            }
            if support == CrateSupport::NotDetected {
                // TODO: check more than one
                support = srcs.into_iter()
                    .map(|src_path| get_crate_support_from_source(&src_path))
                    .next()
                    .unwrap_or(CrateSupport::NotDetected);
            }

            let check = CheckResult {
                package_name: package.name.clone(),
                support,
                active_features: active_features,
            };

            let overall_res = match check.no_std_itself() {
                true => SUCCESS,
                false => FAILURE,
            };
            println!("{}: {}", check.package_name, overall_res);
            if check.std_because_feature() {
                println!("  - Crate supports no_std if \"std\" feature is deactivated.");
                let feat = check.std_feature().unwrap();
                feat.print(&metadata, 2);
            }
            if check.support == CrateSupport::NotDetected {
                println!("  - Did not find a #![no_std] attribute or a conditional attribute #[cfg_attr(not(feature = \"std\"), no_std)] in the crate source. Crate most likely doesn't support no_std without changes.");
            }
        }
        std::process::exit(0);
    }
    app.print_help().unwrap();
}
