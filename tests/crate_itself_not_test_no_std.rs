extern crate assert_cmd;

use assert_cmd::prelude::*;
use std::process::Command;

mod crate_itself_not_test_no_std {
    use super::*;

    #[test]
    fn it_succeeds() {
        Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .unwrap()
            .arg("check")
            .current_dir("./tests/crate_itself_not_test_no_std")
            .assert()
            .success();
    }
}

#[test]
fn it_prints_checkmark() {
    let output = Command::cargo_bin(env!("CARGO_PKG_NAME"))
        .unwrap()
        .arg("check")
        .current_dir("./tests/crate_itself_not_test_no_std")
        .output()
        .unwrap()
        .stdout;
    let output = String::from_utf8(output).unwrap();

    let expected_cause = "crate_itself_not_test_no_std: SUCCESS";
    assert!(output.contains(expected_cause));
}
