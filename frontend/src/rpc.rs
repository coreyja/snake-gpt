use gloo_net::http::{Method, Request};
use shared::playground::ClientTransport;

struct Client;

#[async_trait::async_trait(?Send)]
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

        let req = if let Some(body) = body {
            Request::new(route).method(method).json(&body)?
        } else {
            Request::new(route).method(method)
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
