extern crate hyper;
extern crate openqa;
extern crate futures;

use hyper::rt::{self, Future};
use futures::future;

use openqa::*;

fn run() -> impl Future<Item=(), Error=()> {
    let oqa = OpenQA::with_conf_file("~/.config/openqa/client.conf",
                                     "openqa.suse.de").unwrap();
    let machines = oqa.get_machines();
    println!("Fetching machines from OpenQA.");
    let machines = machines.wait().unwrap().machines;

    println!("Matching Machines: ");
    for arch in &["aarch64", "ppc64le", "s390x", "64bit"] {
        println!("\nFor {}", arch.replace("64bit", "x86_64"));
        let ids = machines.iter().filter_map(|m| {
            if m.name.starts_with(arch) {
                Some((m.id, &m.name, &m.backend))
            } else {
                None
            }
        });

        for (id, name, backend) in ids {
            println!("{} - {} - {}", id, name, backend);
        }
    }

    future::ok(())
}

fn main() {
    rt::run(rt::lazy(run));
}
