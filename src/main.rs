extern crate hyper;
extern crate hyper_tls;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate futures;
extern crate failure;
extern crate crypto;
extern crate time;

use hyper::{Client, Body, Chunk, Uri};
use hyper::client::HttpConnector;
use hyper::rt::{self, Future, Stream};
use hyper_tls::HttpsConnector;
use futures::stream;
use futures::future;
use failure::Error;
use crypto::hmac::Hmac;
use crypto::sha1::Sha1;
use crypto::mac::MacResult;
use time::get_time;

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

type MyClient = Client<HttpsConnector<HttpConnector>>;

const BASEURL: &str = "https://openqa.suse.de/api/v1/";
const KEY: &str = "1234567890ABCDEF"

fn get(c: &MyClient, url: Uri) -> impl Future<Item=Chunk, Error=Error> {
    c.get(url).and_then(|res| {
        println!("GET -> {}", res.status());
        res.into_body().concat2()
    }).map_err(|e| Error::from(e))
}

fn hex_str(bytes: &[u8]) -> String {
    let xmap = "0123456789abcdef".chars().collect();
    let h = String::default();

    for (a, b) in bytes.iter().step_by(2).zip(bytes.iter().skip(1).step_by(2)) {
        h.push(xmap[a >> 4]);
        h.push(xmap[b & 0x0f]);
    }

    h
}

fn hash(url: &Uri, t: &str) -> HeaderValue {
    let mac = Hmac::new(Sha1::new(), KEY.as_bytes());
    mac.input(url.as_bytes());
    mac.input(t.as_bytes());
    HeaderValue::from_str(&hex_str(mac.result().code()))
}

fn post(c: &MyClient, url: Uri, body: Body) -> impl Future<Item=Request, Error=Error> {
    let mut req = Request::new(body);
    *req.method_mut() = Method::POST;
    let hdrs = req.headers_mut();
    hdrs.insert("Accept", HeaderValue::from_str("application/json"));
    let t = format!("{}", get_time().sec);
    hdrs.insert("X-API-Microtime", HeaderValue::from_str(&t).unwrap());
    hdrs.insert("X-API-Key", HeaderValue::from_str(KEY).unwrap());
    hdrs.insert("X-API-Hash", hash(&url, &t));
    *req.uri_mut() = url;

    c.request(req).map_err(|e| Error::from(e))
}

fn run() -> impl Future<Item=(), Error=()> {
    let mut https = HttpsConnector::new(1).unwrap();
    https.force_https(true);
    let c = Client::builder().build::<_, Body>(https);

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
            let sets = test.settings;
            if let Some(s) = sets.iter().find(|s| s.key == "PUBLISH_HDD_1") {
                println!("Found PUBLISH_HDD_1 = {}", s.value);
            }
            let hdd1 = sets.iter().find(|s| s.key == "HDD_1");
            if let Some(s) = sets.iter().find(|s| s.key == "START_AFTER_TEST").and(hdd1) {
                println!("Found HDD_1 = {} and START_AFTER_TEST", s.value);
            }
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
