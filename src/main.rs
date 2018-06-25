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

use std::io::{self, Write, Read};

use bytes::{BufMut, BytesMut};
use http::uri::Uri;
use http::header::HeaderValue;
use hyper::{Client, Body, Chunk};
use hyper::client::HttpConnector;
use hyper::rt::{self, Future, Stream};
use hyper_tls::HttpsConnector;
use futures::future;
use failure::Error;
use crypto::hmac::Hmac;
use crypto::sha1::Sha1;
use crypto::mac::Mac;
use time::get_time;

#[derive(Serialize, Deserialize, Debug)]
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
    base_uri: BytesMut,
    key: String,
    secret: String,
}

const HOST: &str = "http://localhost";
const KEY: &str = "1234567890ABCDEF";
const SECRET: &str = "1234567890ABCDEF";

const XMAP_U: &[u8] = b"0123456789ABCDEF";
const XMAP_L: &[u8] = b"0123456789abcdef";

impl UserAgent {
    fn new<U, S, T>(host: U, key: S, secret: T) -> UserAgent
    where
        BytesMut: From<U>,
        S: Into<String>,
        T: Into<String>,
    {
        let https = HttpsConnector::new(1).unwrap();
        let client = Client::builder().build::<_, Body>(https);
        let mut base_uri = BytesMut::from(host);
        base_uri.extend_from_slice(&b"/api/v1/"[..]);

        UserAgent {
            client,
            base_uri,
            key: key.into(),
            secret: secret.into(),
        }
        
    }

    fn hash(&self, url: &Uri, time: &str) -> HeaderValue {
        let mut mac = Hmac::new(Sha1::new(), self.secret.as_bytes());

        mac.input(url.path().as_bytes());
        if let Some(q) = url.query() {
            mac.input(b"?");
            mac.input(q.as_bytes());
        }
        mac.input(time.as_bytes());

        HeaderValue::from_shared(hex_str(mac.result().code()).into()).unwrap()
    }

    fn post(&self, c: &MyClient, url: Uri) -> impl Future<Item=Chunk, Error=Error> {
        let mut req = http::Request::new(Body::default());
        *req.method_mut() = http::Method::POST;
        {
            let hdrs = req.headers_mut();
            hdrs.insert("Accept", HeaderValue::from_str("application/json").unwrap());
            let t = format!("{}", get_time().sec);
            hdrs.insert("X-API-Microtime", HeaderValue::from_str(&t).unwrap());
            hdrs.insert("X-API-Key", HeaderValue::from_str(&self.key).unwrap());
            hdrs.insert("X-API-Hash", self.hash(&url, &t));
        }
        *req.uri_mut() = url;
        println!("POST {:#?}", req);

        c.request(req).and_then(|res| {
            println!("POST -> {}", res.status());
            res.into_body().concat2()
        }).map_err(|e| Error::from(e))
    }

    fn get(&self, c: &MyClient, url: Uri) -> impl Future<Item=Chunk, Error=Error> {
        c.get(url).and_then(|res| {
            println!("GET -> {}", res.status());
            res.into_body().concat2()
        }).map_err(|e| Error::from(e))
    }

    fn url_bytes(&self, path: &str) -> BytesMut {
        let mut bytes = self.base_uri.clone();
        bytes.extend_from_slice(path.as_bytes());
        bytes
    }

    pub fn url(&self, path: &str) -> Uri {
        Uri::from_shared(self.url_bytes(path).into()).unwrap()
    }

    pub fn url_query(&self, path: &str, pairs: Vec<(&str, &str, bool)>) -> Uri {
        let mut bytes = self.url_bytes(path);
        bytes.extend_from_slice(&b"?"[..]);
        for (k, v, setting) in &pairs {
            if *setting {
                bytes.extend_from_slice(&b"settings%5B"[..]);
            }
            percent_encode(k.as_bytes(), &mut bytes);
            if *setting {
                bytes.extend_from_slice(&b"%5D="[..]);
            } else {
                bytes.extend_from_slice(&b"="[..]);
            }
            percent_encode(v.as_bytes(), &mut bytes);
            bytes.extend_from_slice(&b"&"[..]);
        }
        let l = bytes.len() - 1;
        bytes.truncate(l);

        Uri::from_shared(bytes.into()).unwrap()
    }
}

impl Default for UserAgent {
    fn default() -> UserAgent {
        UserAgent::new(HOST, KEY, SECRET)
    }
}

fn percent_encode(data: &[u8], out: &mut BytesMut) {
    out.reserve(data.len() * 3);
    for b in data {
        match *b {
            b'0' ... b'9' | b'A' ... b'Z' | b'a' ... b'z' | b'-' | b'_' | b'.' | b'~' => {
                out.put(*b);
            },
            b' ' => out.put(b'+'),
            _ => {
                out.put(b'%');
                out.put(XMAP_U[((b >> 4) & 0x0fu8) as usize]);
                out.put(XMAP_U[(b & 0x0fu8) as usize]);
            },
        }
    }
}

fn hex_str(bytes: &[u8]) -> BytesMut {
    let mut h = BytesMut::with_capacity(bytes.len() * 2);

    for b in bytes {
        h.put(XMAP_L[((b >> 4) & 0x0fu8) as usize]);
        h.put(XMAP_L[(b & 0x0fu8) as usize]);
    }

    h
}

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
    let ua = UserAgent::default();

    let mut tests = ua.get(&ua.client, ua.url("test_suites"))
        .and_then(|body: Chunk| {
            let res = serde_json::from_slice::<TestSuites>(&body)
                .map_err(|e| Error::from(e));
            future::result(res)
        })
        .map_err(|e| eprintln!("Failed to get tests: {}", e))
        .map(|tests: TestSuites| tests.TestSuites)
        .wait().unwrap();

    for test in &mut tests {
        println!("Inspecting {}", test.name);
        let mut sets = &mut test.settings;

        let publish_vars = {
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

        if let Some(s) = publish_vars { sets.push(s); }
        if let Some(s) = uefi_vars { sets.push(s); }

        println!("Update: ");
        print_settings(&sets);
        println!("y/n/a? -> ");
        if read_yn() {
            let mut params: Vec<(&str, &str, bool)> = vec![
                ("name", &test.name, false),
                ("description", &test.description, false)
            ];
            for s in sets {
                params.push((&s.key, &s.value, true));
            }
            let res = ua.post(&ua.client,
                              ua.url_query(&format!("test_suites/{}", test.id), params)).wait();
            match res {
                Ok(resp) => {
                    print!("POST Response:\n\t");
                    io::stdout().write_all(&resp).unwrap();
                    println!("");
                },
                Err(e) => eprintln!("Failed to post changes: {}", e),
            }
        }
    }
    future::ok(())
}

fn main() {
    rt::run(rt::lazy(run));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac() {
        let mut mac = Hmac::new(Sha1::new(), b"1234567890ABCDEF");
        let payload = "settings[foo]=bar";
        mac.input(payload.as_bytes());
        let res = mac.result();
        let raw = res.code();
        let hex = hex_str(&raw);
        assert_eq!(raw.len() * 2, hex.len());
        assert_eq!("f4d2e8996c1d68aff0892b248a92651c8d3e9a4c", &hex);
    }

    #[test]
    fn percent_encode() {
        let data = b"`~+_-;:\"?<>{}[]@*&^$#=|/`~+_-;:\"?<>{}[]@*&^$#=|/`~+_-;:\"?<>{}[]@*&^$#=|/'";
        let escaped = "%60~%2B_-%3B%3A%22%3F%3C%3E%7B%7D%5B%5D%40%2A%26%5E%24%23%3D%7C%2F%60~%2B_-%3B%3A%22%3F%3C%3E%7B%7D%5B%5D%40%2A%26%5E%24%23%3D%7C%2F%60~%2B_-%3B%3A%22%3F%3C%3E%7B%7D%5B%5D%40%2A%26%5E%24%23%3D%7C%2F%27";
        let mut buf = BytesMut::default();

        super::percent_encode(data, &mut buf);
        assert_eq!(escaped, &buf);

        let data = b"0123456789`~!@#$%^&*()_-+={}[]|\\abcdefghijklmnopqrstuwvxyzABCDEFGHIJKLMNOPQRSTUVWXYZ:;\"'<>,.?/ ";
        let escaped = "0123456789%60~%21%40%23%24%25%5E%26%2A%28%29_-%2B%3D%7B%7D%5B%5D%7C%5CabcdefghijklmnopqrstuwvxyzABCDEFGHIJKLMNOPQRSTUVWXYZ%3A%3B%22%27%3C%3E%2C.%3F%2F+";
        buf.clear();

        super::percent_encode(data, &mut buf);
        assert_eq!(escaped, &buf);
    }
}
