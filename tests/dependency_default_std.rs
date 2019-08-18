extern crate assert_cmd;

use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn it_fails_with_exit_code_1() {
    Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("check")
        .current_dir("./tests/dependency_default_std")
        .assert()
        .code(1);
}

#[test]
fn it_succeeds_with_no_default_features() {
    Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("check")
        .arg("--no-default-features")
        .current_dir("./tests/dependency_default_std")
        .assert()
        .success();
}

#[test]
fn it_prints_cause() {
    let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("check")
        .current_dir("./tests/dependency_default_std")
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();

    let expected_cause =
        "Caused by implicitly enabled default feature from \"dependency_default_std:0.1.0\"";
    assert!(output.contains(expected_cause));
}
