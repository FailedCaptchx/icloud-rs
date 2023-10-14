use serde::Deserialize;

#[derive(Deserialize)]
pub struct Data {
    pub webservices: Services,
    pub trust_token: Option<Vec<String>>
}

#[derive(Deserialize)]
struct Services {
    drivews: Service
}

#[derive(Deserialize)]
struct Service {
    url: String,
    status: String
}