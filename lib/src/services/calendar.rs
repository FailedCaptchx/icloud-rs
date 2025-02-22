use crate::Service;

impl Service {
    pub async fn fetch_calendars<T>(&self, timezone: T, from: T, to: T) -> reqwest::Result<String>
        where T: Into<String> {
        let lang = self.account.info.lang.clone();
        let point = self.account.webservices.calendar.url.clone();
        let params = [
            ("lang", lang),
            ("usertz", timezone.into()),
            ("startDate", from.into()),
            ("endDate", to.into())
        ];
        let url = reqwest::Url::parse_with_params(
            &format!("{point}/ca/events"),
            &params).unwrap(); // better error handling
        let req = self.client
            .get(url)
            .json(&params).build()?;
        let res = self.client.execute(req).await?;
        Ok(res.text().await?)
    }
}