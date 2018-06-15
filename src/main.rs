extern crate hyper;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate futures;
extern crate failure;

use hyper::{Client, Chunk, Uri};
use hyper::client::HttpConnector;
use hyper::rt::{self, Future, Stream};
use futures::stream;
use futures::future;
use failure::Error;

#[derive(Serialize, Deserialize)]
struct Setting {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize)]
struct TestSuite {
    #[serde(default)]
    pub description: String,
    pub id: i32,
    pub name: String,
    pub settings: Vec<Setting>,
}

#[derive(Serialize, Deserialize)]
struct TestSuites {
    pub TestSuites: Vec<TestSuite>,
}

const BASEURL: &str = "http://openqa.suse.de/api/v1/";

fn get(c: &Client<HttpConnector>, url: Uri) -> impl Future<Item=Chunk, Error=Error> {
    c.get(url).and_then(|res| {
        println!("GET -> {}", res.status());
        res.into_body().concat2()
    }).map_err(|e| Error::from(e))
}

fn run() -> impl Future<Item=(), Error=()> {
    let c = Client::new();

    get(&c, format!("{}{}", BASEURL, "test_suites").parse::<Uri>().unwrap())
        .and_then(|body: Chunk| {
            let res = serde_json::from_slice::<TestSuites>(&body)
                .map_err(|e| Error::from(e));
            future::result(res)
        })
        .map_err(|e| eprintln!("Failed to get tests: {}", e))
        .map(|tests: TestSuites| stream::iter_ok(tests.TestSuites))
        .flatten_stream()
        .for_each(|test| {
            println!("id: {}, name: {}", test.id, test.name);
            future::ok(())
        })
}

fn main() {
    rt::run(rt::lazy(run));
    // For each test case
    // 	If it has a publish HDD_1 generate a publish pflash vars
    //  If it has a HDD_1 and a START_AFTER_TEST, generate a UEFI_PFLASH_VARS
    //  present changes for sanity check
}
