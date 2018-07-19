extern crate http;
extern crate bytes;
extern crate hyper;
extern crate hyper_tls;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate futures;
#[macro_use]
extern crate failure;
extern crate crypto;
extern crate time;
#[macro_use]
extern crate log;
extern crate ini;

pub mod user_agent;

use std::path::Path;

use serde::de::DeserializeOwned;
use bytes::BytesMut;
use futures::future;
use hyper::rt::Future;
use hyper::Chunk;
use failure::Error;
use ini::Ini;

pub use user_agent::UserAgent;

#[derive(Serialize, Deserialize, Debug)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

#[derive(Serialize, Deserialize)]
pub struct TestSuite {
    #[serde(default)]
    pub description: String,
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub settings: Vec<Setting>,
}

#[derive(Serialize, Deserialize)]
pub struct TestSuites {
    #[serde(rename = "TestSuites")]
    pub test_suites: Vec<TestSuite>,
}

#[derive(Deserialize)]
pub struct Product {
    pub id: i32,
    pub arch: String,
    pub distri: String,
    pub flavor: String,
    pub version: String,
    #[serde(default)]
    pub settings: Vec<Setting>,
}

#[derive(Deserialize)]
pub struct Products {
    #[serde(rename = "Products")]
    pub products: Vec<Product>,
}

#[derive(Deserialize)]
pub struct Machine {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub backend: String,
    #[serde(default)]
    pub settings: Vec<Setting>,
}

#[derive(Deserialize)]
pub struct Machines {
    #[serde(rename = "Machines")]
    pub machines: Vec<Machine>,
}

#[derive(Deserialize)]
pub enum UpdateResult {
    #[serde(rename = "result")]
    Ok(i32),
    #[serde(rename = "error")]
    Err(String),
}

#[derive(Deserialize)]
pub enum CreateResult {
    #[serde(rename = "id")]
    Ok(i32),
    #[serde(rename = "error")]
    Err(String),
}

pub struct JobTemplate {
    pub product_id: i32,
    pub machine_id: i32,
    pub group_id: i32,
    pub test_suite_id: i32,
}

#[derive(Deserialize)]
pub struct JobTemplateInfo {
    pub group_name: String,
    pub id: i32,
    pub machine: Machine,
    pub prio: i32,
    pub test_suite: TestSuite,
}

#[derive(Deserialize)]
pub struct JobTemplateInfos {
    #[serde(rename = "JobTemplates")]
    pub job_templates: Vec<JobTemplateInfo>, 
}

pub struct OpenQA {
    ua: UserAgent,
}

impl OpenQA {
    pub fn new<U, S, T>(host: U, key: S, secret: T) -> OpenQA
    where
        BytesMut: From<U>,
        S: Into<String>,
        T: Into<String>,
    {
        OpenQA {
            ua: UserAgent::new(host, key, secret),
        }
    }

    pub fn with_conf_file<P, H>(file_path: P, host: H) -> Result<OpenQA, Error>
    where
        P: AsRef<Path>,
        H: AsRef<str>
    {
        use std::io::Read;
        use std::fs::File;

        let mut path_buf;
        let file_path = match file_path.as_ref().strip_prefix("~") {
            Ok(p) => {
                path_buf = std::env::home_dir()
                    .ok_or(format_err!("Can't get home dir"))?;
                path_buf.push(p);
                &path_buf
            },
            Err(_) => file_path.as_ref(),
        };
        let mut file = File::open(file_path)?;
        let mut conf = String::new();
        file.read_to_string(&mut conf)?;

        OpenQA::with_conf(conf, host)
    }

    pub fn with_conf<P, H>(conf: P, host: H) -> Result<OpenQA, Error>
    where
        P: AsRef<str>,
        H: AsRef<str>
    {
        let host = host.as_ref();
        let conf = Ini::load_from_str(conf.as_ref()).map_err(|e| {
            format_err!("Error parsing config: {}", e)
        })?;
        let sec = conf.section(Some(host)).ok_or_else(|| {
            format_err!("Host section [{}] not found in config", host)
        })?;
        let key = sec.get("key").cloned().ok_or_else(|| {
            format_err!("'key' value not found in [{}]", host)
        })?;
        let secret = sec.get("secret").cloned().ok_or_else(|| {
            format_err!("'secret' value ot found in [{}]", host)
        })?;

        Ok(OpenQA {
            ua: UserAgent::new(format!("https://{}", host), key, secret),
        })
    }

    pub fn get<U, T>(&self, url: U) -> impl Future<Item=T, Error=Error>
    where
        U: AsRef<str>,
        T: DeserializeOwned,
    {
        self.ua.get(self.ua.url(url.as_ref())).and_then(|body: Chunk| {
            let res = serde_json::from_slice(&body)
                .map_err(|e| if let Ok(b) = String::from_utf8(body.to_vec()) {
                            format_err!("Deserializing response: {}, Message body: {}",
                                        e, b)
                        } else {
                            format_err!("Deserializing response: {}", e)
                        });
            future::result(res)
        })
    }

    pub fn get_test_suites(&self) -> impl Future<Item=TestSuites, Error=Error>
    {
        self.get("test_suites")
    }

    pub fn get_products(&self) -> impl Future<Item=Products, Error=Error>
    {
        self.get("products")
    }

    pub fn get_machines(&self) -> impl Future<Item=Machines, Error=Error>
    {
        self.get("machines")
    }

    pub fn get_job_templates(&self) -> impl Future<Item=JobTemplateInfos, Error=Error>
    {
        self.get("job_templates")
    }

    pub fn post<U, T, K, V, P>(&self, url: U, pairs: P) -> impl Future<Item=T, Error=Error>
    where
        U: AsRef<str>,
        T: DeserializeOwned,
        K: AsRef<[u8]>,
        V: AsRef<[u8]>,
        P: AsRef<[(K, V, bool)]>,
    {
        self.ua.post(self.ua.url_query(url.as_ref(), pairs)).and_then(|body: Chunk| {
            let res = serde_json::from_slice(&body)
                .map_err(|e| if let Ok(b) = String::from_utf8(body.to_vec()) {
                            format_err!("Deserializing response: {}, Message body: {}",
                                        e, b)
                        } else {
                            format_err!("Deserializing response: {}", e)
                        });
            future::result(res)
        })
    }

    pub fn upd_test_suite<'a>(&self, test: &'a TestSuite)
                              -> impl Future<Item=UpdateResult, Error=Error> + 'a
    {
        let mut params: Vec<(&str, &str, bool)> = vec![
            ("name", &test.name, false),
            ("description", &test.description, false)
        ];
        for s in &test.settings {
            params.push((&s.key, &s.value, true));
        }

        self.post(format!("test_suites/{}", test.id), params)
    }

    pub fn new_job_template(&self, template: &JobTemplate)
                            -> impl Future<Item=CreateResult, Error=Error>
    {
        let params = vec![
            ("product_id", template.product_id.to_string(), false),
            ("machine_id", template.machine_id.to_string(), false),
            ("group_id", template.group_id.to_string(), false),
            ("test_suite_id", template.test_suite_id.to_string(), false),
            ("prio", 50.to_string(), false),
        ];

        self.post("job_templates", params)
    }
}

impl Default for OpenQA {
    fn default() -> OpenQA {
        OpenQA {
            ua: UserAgent::default(),
        }
    }
}
