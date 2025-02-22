use serde::Deserialize;

#[derive(Deserialize)]
pub struct Account {
    #[serde(rename = "dsInfo")]
    pub info: Info,
    pub webservices: Services,
    pub trust_token: Option<Vec<String>>
}

#[derive(Deserialize)]
pub struct Info {
    #[serde(rename = "fullName")]
    pub name: String,
    #[serde(rename = "lastName")]
    pub last_name: String,
    #[serde(rename = "firstName")]
    pub first_name: String,
    #[serde(rename = "languageCode")]
    pub lang: String,
    #[serde(rename = "countryCode")]
    pub country_code: String,
    pub locale: String,
    #[serde(rename = "appleIdAliases")]
    pub aliases: Vec<String>,
    #[serde(rename = "primaryEmail")]
    pub email: String,
    #[serde(rename = "isManagedAppleID")]
    pub is_managed: bool,
    #[serde(rename = "isPaidDeveloper")]
    pub is_paid_developer: bool,
    pub locked: bool,
}

#[derive(Deserialize)]
pub struct Services { // no schoolwork and ckdeviceservice because of no url/status
    pub notes: Service,
    pub mail: Service,
    pub ckdatabasews: Service,
    pub photosupload: Service,
    pub mcc: Service,
    pub photos: Service,
    pub drivews: Service,
    pub uploadimagews: Service,
    pub cksharews: Service,
    pub findme: Service,
    pub iworkthumbnailws: Service,
    pub mccgateway: Service,
    pub calendar: Service,
    pub docws: Service,
    pub settings: Service,
    pub premiummailsettings: Service,
    pub ubiquity: Service,
    pub keyvalue: Service,
    pub mpp: Service,
    pub archivews: Service,
    pub push: Service,
    pub iwmb: Service,
    pub iworkexportws: Service,
    pub sharedlibrary: Service,
    pub geows: Service,
    pub account: Service,
    pub contacts: Service,
    pub developerapi: Service,
}

#[derive(Deserialize)]
pub struct Service {
    pub url: String,
    pub status: String,
    #[serde(rename = "isMakoAccount")]
    pub is_mako_account: Option<bool>
}