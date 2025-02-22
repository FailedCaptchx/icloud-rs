use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::sync::Arc;
use reqwest::Client;
use reqwest_cookie_store::CookieStoreMutex;
use crate::{Service, Session};
use crate::deserial::Account;

#[derive(Debug, Clone)]
pub struct AuthError<T: Debug>(pub T);

impl<T: Debug> fmt::Display for AuthError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "")
    }
}

impl<T: Debug> Error for AuthError<T> {}

#[must_use = "this `Auth` may be an `Hsa` variant, which should be handled"]
pub enum Auth {
    Ok((Session, Client, Arc<CookieStoreMutex>, Account)),
    Hsa((Session, Client, Arc<CookieStoreMutex>))
}

impl Auth {
    pub async fn hsa(self, code: String) -> Option<Service> {
        match self {
            Auth::Hsa((s, cl, co)) => {
                match s.validate_2fa(&cl, code).await {
                    Ok(account) => {
                        Some(Service {session: s, client: cl, cookies: co, account})
                    }
                    Err(e) => {
                        println!("{}", e);
                        None
                    }
                }
            }
            Auth::Ok((s, cl, co, a)) => Some(Service {session: s, client: cl, cookies: co, account: a})
        }
    }
}