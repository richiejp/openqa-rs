extern crate hyper;
extern crate openqa;
extern crate futures;

use hyper::rt::{self, Future};
use futures::future;

use openqa::*;

fn run() -> impl Future<Item=(), Error=()> {
    let oqa = OpenQA::with_conf_file("~/.config/openqa/client.conf",
                                     "openqa.suse.de").unwrap();
    let products = oqa.get_products();
    println!("Fetching products from OpenQA.");
    let products = products.wait().unwrap().products;

    let ids = products.into_iter().filter_map(|p| {
        if p.distri == "sle" &&
            p.flavor == "Server-DVD" &&
            p.version == "12-SP4" {
                Some((p.id, p.arch))
            } else {
                None
            }
    });

    println!("Matching Product IDs: ");
    for (id, arch) in ids {
        println!("{} - {}", id, arch);
    }

    future::ok(())
}

fn main() {
    rt::run(rt::lazy(run));
}
