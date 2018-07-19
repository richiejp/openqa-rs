extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate futures;
extern crate failure;

extern crate openqa;

use std::collections::BTreeSet;

use futures::future;
use hyper::rt::{self, Future};

use openqa::*;

fn run() -> impl Future<Item=(), Error=()> {
    let oqa = OpenQA::with_conf_file("~/.config/openqa/client.conf",
                                     "openqa.opensuse.org").unwrap();

    let test = 1223;
    let jobs = oqa.get_job_templates().wait().unwrap();
    let mut groups = BTreeSet::new();
    
    for job in jobs.job_templates.into_iter() {
        if job.test_suite.id == test {
            groups.insert(job.group_name);
        }
    }

    for g in groups.iter() {
        println!("{}", g)
    }

    future::ok(())
}

fn main() {
    rt::run(rt::lazy(run));
}
