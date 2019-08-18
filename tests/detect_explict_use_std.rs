extern crate assert_cmd;

use assert_cmd::prelude::*;
use std::process::Command;

#[test]
fn it_fails_with_exit_code_1() {
    Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("check")
        .current_dir("./tests/detect_explicit_use_std")
        .assert()
        .code(1);
}

#[test]
fn it_prints_cause() {
    let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("check")
        .current_dir("./tests/detect_explicit_use_std")
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();

    let expected_cause = "Source code contains an explicit `use std::` statement";
    assert!(output.contains(expected_cause));
}
