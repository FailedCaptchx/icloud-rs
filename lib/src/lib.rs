#![feature(async_closure)]
#![feature(try_trait_v2)]

extern crate core;

mod services;
mod deserial;
pub mod err;

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::sync::Arc;
use reqwest::{Body, Client, IntoUrl, Response};
use reqwest::header::{AsHeaderName, HeaderMap, HeaderValue};
use reqwest_cookie_store::{CookieStore, CookieStoreMutex};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;
use crate::deserial::Account;
use crate::err::Auth;
use crate::err::Auth::Hsa;

const AUTH_ENDPOINT: &str = "https://idmsa.apple.com/appleauth/auth";
const HOME_ENDPOINT: &str = "https://www.icloud.com";
const SETUP_ENDPOINT: &str = "https://setup.icloud.com/setup/ws/1";

#[derive(Serialize, Deserialize, Debug)]
pub struct Session {
    pub client_id: String,
    #[serde(rename = "X-Apple-TwoSV-Trust-Token")]
    pub trust_token: Vec<String>,
    #[serde(rename = "X-Apple-ID-Account-Country")]
    pub account_country: String,
    pub scnt: String,
    #[serde(rename = "X-Apple-ID-Session-Id")]
    pub session_id: String,
    #[serde(rename = "X-Apple-Session-Token")]
    pub session_token: String
}

impl Session {
    pub async fn import_from_string<S, P>(data: S, cookie_path: P) -> Result<Service, Box<dyn Error>>
        where S: Into<String>,
              P: AsRef<Path> {
        let (client, cookies) = Self::client(cookie_path)?;
        let session = serde_json::from_str::<Session>(&data.into())?;
        let account = session.authenticate_with_token(&client).await?;
        //let account = session.validate_token(&client).await?;
        Ok(Service {session, client, cookies, account})
    }
    pub async fn import_from_file<P>(save_path: P, cookie_path: P) -> Result<Service, Box<dyn Error>>
        where P: AsRef<Path> {
        Ok(Self::import_from_string(std::fs::read_to_string(save_path)?, cookie_path).await?)
    }
    pub async fn new<P>(apple_id: String,
                        password: String,
                        cookie_save: P,
                        client_id: Option<String>) -> Result<Auth, Box<dyn Error>>
        where P: AsRef<Path> {
        let (client, cookies) = Self::client(cookie_save)?;
        let client_id = client_id.unwrap_or(format!("auth-{}", Uuid::new_v4()));
        let headers: HeaderMap = get_auth_headers(&client_id);
        let trust_token = vec!();
        let data = json!({
            "accountName": apple_id,
            "password": password,
            "rememberMe": true,
            "trustTokens": trust_token
        });
        let req = client
            .post(format!("{AUTH_ENDPOINT}/signin?isRememberMeEnabled=true"))
            .json(&data)
            .headers(headers)
            .build()?;
        let res = client.execute(req).await?;
        let res_head = res.headers();
        let session = Session {
            client_id: client_id.to_string(),
            trust_token,
            account_country: pull_session(res_head, "X-Apple-ID-Account-Country")
                .unwrap_or("USA".to_string()),
            scnt: pull_session(res_head, "scnt").unwrap_or("".to_string()),
            session_id: pull_session(res_head, "X-Apple-ID-Session-Id").unwrap(),
            session_token: pull_session(res_head, "X-Apple-Session-Token").unwrap(),
        };
        let res = res.text().await?; // get the body of the response
        if let Value::Object(o) = serde_json::from_str(&res).unwrap() {
            if let Some(Value::String(s)) = o.get("authType") { // use body to check if we need to 2fa
                if s == "hsa2" {
                    return Ok(Hsa((session, client, cookies)))
                } //TODO: Missing 2FA of other kinds like by text
            }
        }
        let account = session.authenticate_with_token(&client).await?;
        Ok(Auth::Ok((session, client, cookies, account)))
    }
    async fn validate_2fa(&self, client: &Client, code: String) -> reqwest::Result<Account> {
        let data = json!({
            "securityCode": {"code": code}
        });
        let mut headers = get_auth_headers(&self.client_id);
        headers.insert(
            "scnt",
            HeaderValue::from_str(&self.scnt.as_str()).unwrap()
        );
        headers.insert(
            "X-Apple-ID-Session-Id",
            HeaderValue::from_str(&self.session_id.as_str()).unwrap()
        );
        let req = client
            .post(format!("{AUTH_ENDPOINT}/verify/trusteddevice/securitycode"))
            .json(&data)
            .headers(headers)
            .build()?;
        client.execute(req).await?;
        let data = self.trust_session(client).await?;
        Ok(data)
    }
    pub async fn trust_session(&self, client: &Client) -> reqwest::Result<Account> {
        let mut headers = get_auth_headers(&self.client_id);
        headers.insert(
            "scnt",
            HeaderValue::from_str(&self.scnt.as_str()).unwrap()
        );
        headers.insert(
            "X-Apple-ID-Session-Id",
            HeaderValue::from_str(&self.session_id.as_str()).unwrap()
        );
        let req = client.post(format!("{AUTH_ENDPOINT}/2sv/trust")).headers(headers).build()?;
        client.execute(req).await?;
        Ok(self.authenticate_with_token(client).await?)
    }
    async fn authenticate_with_token(&self, client: &Client) -> reqwest::Result<Account> {
        let data = json!({
            "accountCountryCode": self.account_country,
            "dsWebAuthToken": self.session_token,
            "extended_login": true,
            "trustToken": self.trust_token,
        });
        let req = client
            .post(format!("{SETUP_ENDPOINT}/accountLogin"))
            .json(&data).build()?;
        let res = client.execute(req).await?; // WHY IS IT NOT SAVING COOKIES??????
        let _ = res.cookies().map(|c| println!("{:?}", c.name()));
        println!("{:?}", res.headers());
        Ok(res.json().await?)
    }
    async fn authenticate_with_credentials(&self, client: &Client, apple_id: String, password: String, service: String) -> reqwest::Result<()> {
        let data = json!({
            "appName": service,
            "apple_id": apple_id,
            "password": password,
        });
        let req = client
            .post(format!("{SETUP_ENDPOINT}/accountLogin"))
            .json(&data).build()?;
        client.execute(req).await?;
        Ok(())
    }
    async fn validate_token(&self, client: &Client) -> reqwest::Result<Account> { // DO NOT USE - PROBLEMATIC
        let req = client
            .post(format!("{SETUP_ENDPOINT}/validate"))
            .body("null")
            .build()?;
        let res = client.execute(req).await?;
        Ok(res.error_for_status()?.json().await?)
    }
    fn client<P: AsRef<Path>>(cookie_save: P) -> Result<(Client, Arc<CookieStoreMutex>), Box<dyn Error>> {
        let headers_hashmap: HashMap<String, String> = [
            ("Origin", HOME_ENDPOINT),
            ("Referer", &format!("{HOME_ENDPOINT}/"))
        ].iter().map(|(k,v)| (k.to_string(), v.to_string())).collect();
        let cookies = {
            if let Ok(file) = File::open(cookie_save)
                .map(BufReader::new) {
                CookieStore::load_json(file).unwrap()
            } else {
                CookieStore::new(None)
            }
        };
        let cookies = CookieStoreMutex::new(cookies);
        let cookies = Arc::new(cookies);
        let cookie_access = std::sync::Arc::clone(&cookies);
        Ok((Client::builder()
            .cookie_store(true)
            .cookie_provider(cookies)
            .default_headers(HeaderMap::try_from(&headers_hashmap).unwrap())
            .build()?, cookie_access))
    }
}
fn pull_session<T>(map: &HeaderMap, key: T) -> Option<String>
    where T: AsHeaderName
{
    Some(map.get(key)?.to_str().unwrap().to_string())
}
fn get_auth_headers(client_id: &str) -> HeaderMap {
    let headers: HashMap<String, String> = [
        ("Accept", "application/json"),
        ("Content-Type", "application/json"),
        ("X-Apple-OAuth-Client-Id", "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d"),
        ("X-Apple-OAuth-Client-Type", "firstPartyAuth"),
        ("X-Apple-OAuth-Redirect-URI", "https://www.icloud.com"),
        ("X-Apple-OAuth-Require-Grant-Code", "true"),
        ("X-Apple-OAuth-Response-Mode", "web_message"),
        ("X-Apple-OAuth-Response-Type", "code"),
        ("X-Apple-OAuth-State", client_id),
        ("X-Apple-Widget-Key", "d39ba9916b7251055b22c7f910e2ea796ee65e98b2ddecea8f5dde8d9d1a815d")
    ].iter().map(|(k,v)| (k.to_string(), v.to_string())).collect();
    HeaderMap::try_from(&headers).unwrap()
}
pub struct Service {
    session: Session,
    client: Client,
    cookies: Arc<CookieStoreMutex>,
    account: Account
}
impl Service {
    pub async fn get_name(&self) -> String {
        self.account.info.name.clone()
    }
    pub async fn get_email(&self) -> String {
        self.account.info.email.clone()
    }
    pub async fn auth(&self, apple_id: String, password: String) {
        let _ = self.session.authenticate_with_credentials(&self.client, apple_id, password, "calendar".to_string()).await;
    }
    pub fn save_cookies(&self) {
        let mut writer = File::create("cookies.json") //TODO: USE REAL COOKIES FOLDER
            .map(std::io::BufWriter::new)
            .unwrap();
        let cookies = self.cookies.lock().unwrap();
        cookies.save_json(&mut writer).unwrap();
    }
    pub fn serialize_session(&self) -> serde_json::Result<String> {
        Ok(serde_json::to_string(&self.session)?)
    }
}

struct Testing {}

impl Testing {
    async fn post<S, T>(client: &Client, url: S, data: T) -> reqwest::Result<Response>
        where S: IntoUrl, T: Into<Body> {
        let req = client
            .post(url)
            .body(data).build()?;
        let res = client.execute(req).await?;
        println!("{:?}", res.headers());
        Ok(res)
    }
    async fn post_json<S>(client: &Client, url: S, data: Value) -> reqwest::Result<Response>
        where S: IntoUrl {
        let req = client
            .post(url)
            .json(&data).build()?;
        let res = client.execute(req).await?;
        println!("{:?}", res.headers());
        Ok(res)
    }
}