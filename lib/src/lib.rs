mod drive;
mod deserial;

use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::sync::Arc;
use reqwest::Client;
use reqwest::header::{AsHeaderName, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use crate::deserial::Data;

const AUTH_ENDPOINT: &str = "https://idmsa.apple.com/appleauth/auth";
const HOME_ENDPOINT: &str = "https://www.icloud.com";
const SETUP_ENDPOINT: &str = "https://setup.icloud.com/setup/ws/1";

pub struct User {
    apple_id: String,
    password: String
}

#[derive(Deserialize, Debug)]
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
    async fn validate_token(&self, client: &Client) -> reqwest::Result<Data> { //TODO: fix
        let req = client
            .post(format!("{SETUP_ENDPOINT}/validate"))
            .build()?;
        let res = client.execute(req).await?;
        Ok(res.error_for_status()?.json().await?)
    }
}

///Base authentication class for iCloud services.
pub struct Service {
    user: User,
    client: Client,
    cookie_dir: Option<String>,
    session: Session,
    client_id: String,
    data: Option<Data>
}

impl Service {
    pub async fn new(apple_id: String,
                     password: String,
                     cookie_dir: Option<String>,
                     client_id: Option<String>) -> reqwest::Result<Self> {
        let headers_hashmap: HashMap<String, String> = [
            ("Origin", HOME_ENDPOINT),
            ("Referer", &format!("{HOME_ENDPOINT}/"))
        ].iter().map(|(k,v)| (k.to_string(), v.to_string())).collect();
        let cookies = {
            if let Ok(file) = File::open(dirs::config_dir().unwrap().join("nefos-lib").join("cookies.json"))
                .map(BufReader::new) {
                reqwest_cookie_store::CookieStore::load_json(file).unwrap()
            } else {
                reqwest_cookie_store::CookieStore::new(None)
            }
        };
        let cookies = reqwest_cookie_store::CookieStoreMutex::new(cookies);
        let cookies = Arc::new(cookies);
        let client = Client::builder()
            .cookie_store(true)
            .cookie_provider(cookies)
            .default_headers(HeaderMap::try_from(&headers_hashmap).unwrap())
            .build()?;
        let user = User { apple_id, password };
        let mut client_id = client_id.unwrap_or(format!("auth-{}", Uuid::new_v4()));
        let mut session: Session;
        if let Ok(c) = std::fs::read_to_string(dirs::config_dir().unwrap().join("nefos-lib").join("session.json")) {
            session = serde_json::from_str(&c).unwrap();
            if let Err(e) = session.validate_token(&client).await {
                println!("{}", e);
                session = Self::authenticate(&client, &client_id, &user, vec!()).await.unwrap();
            }
            if client_id.starts_with("auth-") {
                client_id = session.client_id.clone();
            }
        } else {
            session = Self::authenticate(&client, &client_id, &user, vec!()).await.unwrap();
        }
        Ok(Service {
            user,
            client,
            cookie_dir,
            session,
            client_id,
            data: None
        })
    }
    async fn authenticate(client: &Client, client_id: &str, user: &User, trust_token: Vec<String>)
        -> Result<Session, Box<dyn Error>> {
        let headers: HeaderMap = Self::get_auth_headers(client_id);
        let data = json!({
            "accountName": user.apple_id,
            "password": user.password,
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
        let session = Session { //TODO: set up the session struct
            client_id: "".to_string(),
            trust_token,
            account_country: "".to_string(),
            scnt: "".to_string(),
            session_id: "".to_string(),
            session_token: "".to_string(),
        };
        println!("{:?}", res_head);
        let res = res.text().await?;
        /*if let Value::Object(o) = serde_json::from_str(&res).unwrap() {
            if let Some(Value::String(s)) = o.get("authType") {
                if s == "hsa2" {
                    let mut input = [0; 6];
                    print!("Code: ");
                    std::io::stdin().read_exact(&mut input).unwrap();
                    self.validate_2fa(String::from_utf8(input.to_vec())?).await.unwrap();
                } //TODO: Missing 2FA of other kinds like by text
            }
        }*/
        Ok(session)
    }
    async fn authenticate_with_token(&mut self) -> reqwest::Result<()> {
        println!("{:?}", self.session);
        let data = json!({
            "accountCountryCode": self.session.account_country,
            "dsWebAuthToken": self.session.session_token,
            "extended_login": true,
            "trustToken": self.session.trust_token,
        });
        let req = self.client
            .post(format!("{SETUP_ENDPOINT}/accountLogin"))
            .json(&data).build()?;
        let res = self.client.execute(req).await?;
        let res = res.text().await?;
        println!("{}", res);
        Ok(())
    }
    ///Login to a service with email and password (when there is no session or it has died)
    async fn authenticate_with_credentials(&mut self, service: String) -> reqwest::Result<()> {
        let data = json!({
            "appName": service,
            "apple_id": self.user.apple_id,
            "password": self.user.password,
        });
        let req = self.client
            .post(format!("{SETUP_ENDPOINT}/accountLogin"))
            .json(&data).build()?;
        self.client.execute(req).await?;
        self.data = Some(self.session.validate_token(&self.client).await?);
        Ok(())
    }
    pub async fn validate_2fa(&self, code: String) -> reqwest::Result<()> {
        let data = json!({
            "securityCode": {"code": code}
        });
        let mut headers = Self::get_auth_headers(&self.client_id);
        headers.insert(
            "scnt",
            HeaderValue::from_str(&self.session.scnt.as_str()).unwrap()
        );
        headers.insert(
            "X-Apple-ID-Session-Id",
            HeaderValue::from_str(&self.session.session_id.as_str()).unwrap()
        );
        let req = self.client
            .post(format!("{AUTH_ENDPOINT}/verify/trusteddevice/securitycode"))
            .json(&data)
            .headers(headers)
            .build()?;
        let res = self.client.execute(req).await?;
        println!("{}", res.text().await?);
        Ok(())
    }
    fn pull_session<T>(map: &mut HeaderMap, key: T) -> String
    where T: AsHeaderName
    {
        return map.remove(key).unwrap().to_str().unwrap().to_string();
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
}