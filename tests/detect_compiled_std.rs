extern crate assert_cmd;
extern crate predicates;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use std::process::Command;

#[test]
fn check_compiled_requires_nightly() {
    Command::main_binary()
        .unwrap()
        .arg("check-compiled")
        .current_dir("./tests/simple_std_crate")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Not running with Rust nightly!"));
}

#[test]
fn check_compiled_detects_std() {
    Command::new("cargo")
        .args(&[
            "+nightly-2018-12-18",
            "build",
            "-Z",
            "unstable-options",
            "--build-plan",
        ])
        .current_dir("./tests/simple_std_crate")
        .assert()
        .success();

    Command::main_binary()
        .unwrap()
        .arg("check-compiled")
        .current_dir("./tests/simple_std_crate")
        .assert()
        .code(1);
}
