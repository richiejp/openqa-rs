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
use http::header::HeaderValue;
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
use crypto::mac::{Mac, MacResult};
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
    base_uri: Uri,
    key: String,
    secret: String,
}

const KEY: &str = "1234567890ABCDEF";

impl Default for UserAgent {
    fn default() -> UserAgent {
        let mut https = HttpsConnector::new(1).unwrap();
        let client = Client::builder().build::<_, Body>(https);

        UserAgent {
            client,
            base_uri: "https://openqa.suse.de/api/v1/".parse().unwrap(),
            key: KEY.to_string(),
            secret: KEY.to_string(),
        }
    }
}

impl UserAgent {
    fn url_parts(&self, path: &str) -> Parts {
        let parts = Parts::from(self.base_uri.clone());
        {
            let bytes = parts.path_and_query.unwrap().into_bytes();
            bytes.extend_from_slice(path.as_bytes());
        }
        parts
    }

    pub fn url(&self, path: &str) -> Uri {
        Uri::from_parts(self.url_parts(path)).unwrap()
    }

    pub fn url_query(&self, path: &str, pairs: Vec<(&str, &str)>) -> Uri {
        let parts = self.url_parts(path);
        {
            let bytes = parts.path_and_query.unwrap().into_bytes();
            bytes.extend_from_slice(&b"?"[..]);
            for (k, v) in &pairs {
                bytes.extend_from_slice(urlenc::encode(k).as_bytes());
                bytes.extend_from_slice(&b"="[..]);
                bytes.extend_from_slice(urlenc::encode(v).as_bytes());
            }
        }

        Uri::from_parts(parts).unwrap()
    }
}

fn get(c: &MyClient, url: Uri) -> impl Future<Item=Chunk, Error=Error> {
    c.get(url).and_then(|res| {
        println!("GET -> {}", res.status());
        res.into_body().concat2()
    }).map_err(|e| Error::from(e))
}

fn hex_str(bytes: &[u8]) -> String {
    let xmap: Vec<char> = "0123456789abcdef".chars().collect();
    let h = String::default();

    for (a, b) in bytes.iter().step_by(2).zip(bytes.iter().skip(1).step_by(2)) {
        h.push(xmap[((a >> 4) & 0x0fu8) as usize]);
        h.push(xmap[(b & 0x0fu8) as usize]);
    }

    h
}

fn hash(url: &Uri, t: &str) -> HeaderValue {
    let mac = Hmac::new(Sha1::new(), KEY.as_bytes());

    if let Some(s) = url.scheme_part() {
        mac.input(s.as_str().as_bytes());
        mac.input(b"://");
    }
    if let Some(a) = url.authority_part() {
        mac.input(a.as_str().as_bytes());
    }
    mac.input(url.path().as_bytes());
    if let Some(q) = url.query() {
        mac.input(b"?");
        mac.input(q.as_bytes());
    }

    HeaderValue::from_str(&hex_str(mac.result().code())).unwrap()
}

fn post(c: &MyClient, url: Uri) -> impl Future<Item=Chunk, Error=Error> {
    let mut req = http::Request::new(Body::default());
    *req.method_mut() = http::Method::POST;
    let hdrs = req.headers_mut();
    hdrs.insert("Accept", HeaderValue::from_str("application/json").unwrap());
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
        key: key.to_string(),
        value: format!("{}-uefi.qcow2", template.trim_right_matches(".qcow2")),
    }
}

fn read_yn() -> bool {
    use std::io::Read;

    let buf: &mut [u8;1];
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

    let post_res = Vec::default();
    for test in tests {
        let mut sets = &test.settings;
        let mut update = false;

        let pub_hdd = sets.iter().find(|s| s.key == "PUBLISH_HDD_1");
        match pub_hdd {
            Some(s) if s.value.ends_with(".qcow2") => {
                println!("Found PUBLISH_HDD_1 = {}", s.value);
                sets.push(create_uefi_setting("PUBLISH_PFLASH_VARS", &s.value));
                update = true;
            },
            Some(s) => println!("Ignoring PUBLISH_HDD_1 = {}", s.value),
            _ => (),
        }

        let hdd1 = sets.iter().find(|s| s.key == "HDD_1");
        let parent = sets.iter().find(|s| s.key == "START_AFTER_TEST");
        match (hdd1, parent) {
            (Some(s), Some(_)) if s.value.ends_with(".qcow2") => {
                println!("Found HDD_1 = {} and START_AFTER_TEST", s.value);
                sets.push(create_uefi_setting("UEFI_PFLASH_VARS", &s.value));
                update = true;
            },
            (Some(s), _) => println!("Ignoring HDD_1 = {}", s.value),
            _ => (),
        }

        if !update {
            continue;
        }

        println!("Update:\n\tUEFI_PFLASH_VARS = {}\n\t PUBLISH_PFLASH_VARS = {}",
                 sets[sets.len()-1].value, sets[sets.len()-2].value);
        println!("y/n? -> ");
        if read_yn() {
            let params: Vec<(&str, &str)> = vec![("name", &test.name),
                                                 ("description", &test.description)];
            for s in sets {
                params.push((&s.key, &s.value));
            }
            post_res.push(post(&ua.client, ua.url_query(&format!("test_suites/{}", test.id), params))
                          .map_err(|e| eprintln!("Failed to post changes: {}", e)));
        }
    }

    stream::futures_unordered(post_res).for_each(|resp| {
        println!("Posted:");
        
    })
}

fn main() {
    rt::run(rt::lazy(run));
}
