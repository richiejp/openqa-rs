extern crate http;
extern crate bytes;
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
extern crate urlencoding;

use http::uri::{Uri, Authority, Scheme, Parts, PathAndQuery};
use bytes::Bytes;
use hyper::{Client, Body, Chunk};
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
use urlencoding as urlenc;

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

struct UserAgent {
    client: MyClient,
    parts: Parts,
    key: String,
    secret: String,
}

const KEY: &str = "1234567890ABCDEF";
const API: &str = "/api/v1/";

impl Default for UserAgent {
    fn default() -> UserAgent {
        let mut https = HttpsConnector::new(1).unwrap();
        let client = Client::builder().build::<_, Body>(https);

        let parts = Parts::default();
        parts.scheme = Some(Scheme::HTTPS);
        parts.authority = Some("openqa.suse.de".parse().unwrap());

        UserAgent {
            client,
            parts,
            key: KEY.clone(),
            secret: KEY.clone(),
        }
    }
}

impl UserAgent {
    fn urlb(&self, path: &str) -> Vec<u8> {
        let bytes = API.as_bytes().owned();
        bytes.extend_from_slice(path.as_bytes())
    }

    pub fn url(&self, path: &str) -> Uri {
        let parts = self.parts.clone();
        let bytes = Bytes::from(self.urlb(path));
        parts.path_and_query = Some(PathAndQuery::from_shared(bytes).unwrap());
        Uri::from_parts(parts)
    }

    pub fn url_query(&self, path: &str, pairs: Vec<(&str, &str)>) -> Uri {
        let parts = self.parts.clone();
        let bytes = urlb(path);
        bytes.push(b'?');
        for (k, v) in &pairs {
            bytes.extend_from_slice(urlenc::encode(k).as_bytes());
            bytes.push(b'=');
            bytes.extend_from_slice(urlenc::encode(v).as_bytes());
        }

        let bytes = Bytes::from(bytes);
        parts.path_and_query = Some(PathAndQuery::from_shared(bytes).unwrap());
        Uri::from_parts(parts)
    }
}

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

    c.request(req).and_then(|res| {
        println!("POST -> {}", res.status());
        res.into_body().concat2()
    }).map_err(|e| Error::from(e))
}

fn create_uefi_setting(key: &str, template: &str) -> Setting {
    Setting {
        key: key.owned()
        value: format!("{}-uefi.qcow2", template.trim_right_matches(".qcow2")),
    }
}

fn read_yn() -> bool {
    let mut buf: &[u8:1];
    let mut sin = std::io::stdin().lock();

    sin.read_exact(buf).unwrap();
    println!("");
    buf == b"y"
}

fn run() -> impl Future<Item=(), Error=()> {
    let ua = UserAgent::default();

    let tests = get(&ua.client, ua.url("test_suites"))
        .and_then(|body: Chunk| {
            let res = serde_json::from_slice::<TestSuites>(&body)
                .map_err(|e| Error::from(e));
            future::result(res)
        })
        .map_err(|e| eprintln!("Failed to get tests: {}", e))
        .map(|tests: TestSuites| tests.TestSuites)
        .wait().unwrap();

    for test in tests {
        let mut sets = &test.settings;
        let mut update = false;
        
        let pub_hdd = sets.iter().find(|s| s.key == "PUBLISH_HDD_1");
        match pub_hdd {
            Some(s) if s.ends_with(".qcow2") => {
                println!("Found PUBLISH_HDD_1 = {}", s.value);
                sets.push(create_uefi_setting("PUBLISH_PFLASH_VARS", s.value));
                update = true;
            },
            Some(s) => println!("Ignoring PUBLISH_HDD_1 = {}", s.value),
            _ => (),
        }

        let hdd1 = sets.iter().find(|s| s.key == "HDD_1");
        let parent = sets.iter().find(|s| s.key == "START_AFTER_TEST");
        match (hdd1, parent) {
            (Some(s), Some(_)) if s.ends_with(".qcow2") => {
                println!("Found HDD_1 = {} and START_AFTER_TEST", s.value);
                sets.push(create_uefi_setting("UEFI_PFLASH_VARS", s.value));
                update = true;
            },
            (Some(s), _) => println!("Ignoring HDD_1 = {}", s.value),
            _ => (),
        }

        if !update {
            continue;
        }
        
        println!("Update:\n\tUEFI_PFLASH_VARS = {}\n\t PUBLISH_PFLASH_VARS = {}",
                 sets[-1].value, sets[-2].value);
        println!("y/n? -> ");
        if read_yn() {
            let params = vec![("name", test.name),
                              ("description", test.description)];
            for s in sets {
                params.push(s.key, s.value);
            }
            post(&ua.client, ua.url_query(format!("test_suites/{}", test.id), params))
                .map_err(|e| eprintln!("Failed to post changes: {}", e))
                .wait();
        }
    }

}

fn main() {
    rt::run(rt::lazy(run));
}
