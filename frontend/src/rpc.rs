use gloo_net::http::{Method, Request};
use shared::playground::ClientTransport;

#[derive(Debug, Default)]
pub struct Client {
    pub base_url: Option<String>,
}

impl Client {
    pub fn new(base_url: Option<String>) -> Self {
        Self { base_url }
    }
}

impl ClientTransport for Client {
    type Error = gloo_net::Error;

    async fn send_request(
        &self,
        method: &str,
        route: &str,
        body: Option<serde_json::Value>,
    ) -> Result<Result<serde_json::Value, serde_json::Value>, Self::Error> {
        let method = {
            let method = method.to_lowercase();
            let method = match method.as_str() {
                "get" => Method::GET,
                "post" => Method::POST,
                "put" => Method::PUT,
                "delete" => Method::DELETE,
                _ => panic!("Unknown method: {}", method),
            };
            method
        };

        let url = if let Some(base_url) = &self.base_url {
            format!("{}{}", base_url, route)
        } else {
            route.to_string()
        };

        let req = Request::new(&url).method(method);

        let req = if let Some(body) = body {
            req.json(&body)?
        } else {
            req
        };

        let resp = req.send().await?;
        let json = resp.json().await?;

        if resp.status() == 200 {
            Ok(Ok(json))
        } else {
            Ok(Err(json))
        }
    }
}
