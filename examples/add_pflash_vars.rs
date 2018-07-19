extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate futures;
extern crate failure;
extern crate env_logger;

extern crate openqa;

use std::io::{self, Read};

use futures::future;
use hyper::rt::{self, Future};

use openqa::*;

fn create_uefi_setting(key: &str, template: &str) -> Setting {
    Setting {
        key: key.to_string(),
        value: format!("{}-uefi-vars.qcow2", template.trim_right_matches(".qcow2")),
    }
}

fn read_yn() -> bool {
    let buf = &mut [0u8;2];
    let stdin = io::stdin();
    let mut sin = stdin.lock();

    sin.read_exact(buf).unwrap();
    if &buf[0..1] == b"a" {
        panic!("Aborted by user");
    }
    &buf[0..1] == b"y"
}

fn print_settings(settings: &[Setting]) {
    for Setting { key: k, value: v } in settings {
        println!("\t{:20}={}", k, v);
    }
}

fn run() -> impl Future<Item=(), Error=()> {
    let oqa = OpenQA::with_conf_file("~/.config/openqa/client.conf",
                                     "openqa.opensuse.org").unwrap();

    let mut tests = oqa.get_test_suites()
        .map(|tests: TestSuites| tests.test_suites)
        .wait().unwrap();

    for test in &mut tests {
        println!("Inspecting {}", test.name);

        let publish_vars = {
            let mut sets = &mut test.settings;
            let pub_hdd = sets.iter().find(|s| s.key == "PUBLISH_HDD_1");
            let pub_vars = sets.iter().find(|s| s.key == "PUBLISH_PFLASH_VARS");
            match (pub_hdd, pub_vars) {
                (_, Some(v)) => {
                    println!("Found existing PUBLISH_PFLASH_VARS = {}", &v.value);
                    None
                },
                (Some(s), _) if s.value.ends_with(".qcow2") => {
                    println!("Found PUBLISH_HDD_1 = {}", s.value);
                    Some(create_uefi_setting("PUBLISH_PFLASH_VARS", &s.value))
                },
                (Some(s), _) => {
                    println!("Ignoring PUBLISH_HDD_1 = {}", s.value);
                    None
                },
                _ => None,
            }
        };

        let uefi_vars = {
            let mut sets = &mut test.settings;
            let hdd1 = sets.iter().find(|s| s.key == "HDD_1");
            let parent = sets.iter().find(|s| s.key == "START_AFTER_TEST");
            let vars = sets.iter().find(|s| s.key == "UEFI_PFLASH_VARS");
            match (hdd1, parent, vars) {
                (_, _, Some(v)) => {
                    println!("Found existing UEFI_PFLASH_VARS = {}", &v.value);
                    None
                },
                (Some(s), Some(_), _) if s.value.ends_with(".qcow2") => {
                    println!("Found HDD_1 = {} and START_AFTER_TEST", s.value);
                    Some(create_uefi_setting("UEFI_PFLASH_VARS", &s.value))
                },
                (Some(s), _, _) => {
                    println!("Ignoring HDD_1 = {}", s.value);
                    None
                },
                _ => None,
            }
        };

        if publish_vars.is_none() && uefi_vars.is_none() {
            continue;
        }

        if let Some(s) = publish_vars { test.settings.push(s); }
        if let Some(s) = uefi_vars { test.settings.push(s); }

        println!("Update: ");
        print_settings(&test.settings);
        println!("y/n/a? -> ");
        if read_yn() {
            let res = oqa.upd_test_suite(&test).wait().unwrap();

            match res {
                UpdateResult::Ok(resp) => println!("POST Response -> {}", resp),
                UpdateResult::Err(e) => eprintln!("Failed to update test: {}", e),
            }
        }
    }
    future::ok(())
}

fn main() {
    env_logger::init();
    rt::run(rt::lazy(run));
}
