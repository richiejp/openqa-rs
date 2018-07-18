extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate futures;
extern crate failure;

extern crate openqa;

use futures::future;
use hyper::rt::{self, Future};

use openqa::*;

fn run() -> impl Future<Item=(), Error=()> {
    let oqa = OpenQA::with_conf_file("~/.config/openqa/client.conf",
                                     "openqa.suse.de").unwrap();

    let prod_machines: [(i32, i32);3] = [(385, 60), (397, 95), (399, 94)];
    let tests: [i32;11] = [2189, 2697, 2199, 2625, 2197, 2198, 2205, 2178,
                           2183, 2184, 2192];

    for test in &tests {
        for (prod, machine) in &prod_machines {
            let jt = JobTemplate {
                product_id: *prod,
                machine_id: *machine,
                group_id: 158,
                test_suite_id: *test,
            };

            match oqa.new_job_template(&jt).wait().unwrap() {
                CreateResult::Ok(id) => {
                    println!("Created new job template: id={}, test={}, product={}, machine={}",
                             id, test, prod, machine);
                },
                CreateResult::Err(err) => {
                    println!("Failed to create job template; test={}, product={}, machine={}: {}",
                             test, prod, machine, err);
                    return future::err(());
                }
            }
        }
    }

    future::ok(())
}

fn main() {
    rt::run(rt::lazy(run));
}
