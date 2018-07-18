extern crate hyper;
extern crate openqa;
extern crate futures;

use hyper::rt::{self, Future};
use futures::future;

use openqa::*;

fn run() -> impl Future<Item=(), Error=()> {
    let oqa = OpenQA::with_conf_file("~/.config/openqa/client.conf",
                                     "openqa.suse.de").unwrap();

    let names = ["aio_stress", "aiodio", "fs", "io", "can", "cap_bounds",
                 "commands", "connectors", "containers", "controllers",
                 "cpuhotplug", "mm", "pty", "sched", "timers", "tracing",
                 "fs_perms_simple", "input", "kernel_misc"];
    let tests = oqa.get_test_suites();
    println!("Fetching some LTP tests from OpenQA.");
    let tests = tests.wait().unwrap().test_suites;

    let ids = tests.into_iter().filter_map(|t| {
        if t.name.starts_with("ltp_") && names.iter().any(|n| t.name[4..].starts_with(n)) {
            Some((t.id, t.name, t.description))
        } else {
            None
        }
    });

    println!("Matching Tests: ");
    for (id, name, description) in ids {
        println!("{} - {} - {}", id, name, description);
    }

    future::ok(())
}

fn main() {
    rt::run(rt::lazy(run));
}
