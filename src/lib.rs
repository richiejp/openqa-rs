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
#[macro_use]
extern crate log;

pub mod user_agent;

use bytes::BytesMut;
use futures::future;
use hyper::rt::Future;
use hyper::Chunk;
use failure::Error;

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
    pub settings: Vec<Setting>,
}

#[derive(Serialize, Deserialize)]
pub struct TestSuites {
    #[serde(rename = "TestSuites")]
    pub test_suites: Vec<TestSuite>,
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

    pub fn get_test_suites(&self) -> impl Future<Item=TestSuites, Error=Error>
    {
        self.ua.get(self.ua.url("test_suites")).and_then(|body: Chunk| {
            let res = serde_json::from_slice::<TestSuites>(&body)
                .map_err(|e| Error::from(e));
            future::result(res)
        })
    }

    pub fn upd_test_suite(&self, test: &TestSuite) -> impl Future<Item=UpdateResult, Error=Error>
    {
        let mut params: Vec<(&str, &str, bool)> = vec![
            ("name", &test.name, false),
            ("description", &test.description, false)
        ];
        for s in &test.settings {
            params.push((&s.key, &s.value, true));
        }

        self.ua.post(self.ua.url_query(&format!("test_suites/{}", test.id), params))
            .and_then(|body: Chunk| {
                let res = serde_json::from_slice(&body)
                    .map_err(|e| Error::from(e));
                future::result(res)
            })
    }

    pub fn new_job_template(&self, template: &JobTemplate)
                            -> impl Future<Item=CreateResult, Error=Error>
    {
        let params = vec![
            ("product_id", template.product_id.to_string(), false),
            ("machine_id", template.machine_id.to_string(), false),
            ("group_id", template.group_id.to_string(), false),
            ("test_suite_id", template.test_suite_id.to_string(), false)
        ];

        self.ua.post(self.ua.url_query("job_templates", params))
            .and_then(|body: Chunk| {
                let res = serde_json::from_slice(&body)
                    .map_err(|e| Error::from(e));
                future::result(res)
            })
    }
}

impl Default for OpenQA {
    fn default() -> OpenQA {
        OpenQA {
            ua: UserAgent::default(),
        }
    }
}
