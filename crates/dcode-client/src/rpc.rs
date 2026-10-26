use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    Error, HostEndpoint, Result, handshake,
    socket::{close, expect, open, send, within},
};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    result: Value,
    error: Option<ResponseError>,
}

#[derive(Deserialize)]
struct ResponseError {
    code: String,
    message: String,
}

pub async fn call(host: &HostEndpoint, method: &str, params: Value) -> Result<Value> {
    let (mut socket, ack) = open(&host.websocket_url, handshake::rpc_open_frame()).await?;
    if !handshake::host_serves(&ack, method) {
        return Err(Error::Unsupported(method.to_owned()));
    }
    send(&mut socket, &request_frame(method, &params)).await?;
    let response = within(
        RESPONSE_TIMEOUT,
        "response",
        expect(&mut socket, "response"),
    )
    .await?;
    close(&mut socket).await;
    let response: Response = serde_json::from_value(response)?;
    match response.error {
        Some(error) => Err(Error::Rpc {
            method: method.to_owned(),
            code: error.code,
            message: error.message,
        }),
        None => Ok(response.result),
    }
}

fn request_frame(method: &str, params: &Value) -> Value {
    json!({
        "kind": "request",
        "requestId": uuid::Uuid::new_v4().to_string(),
        "method": method,
        "schemaVersion": handshake::schema_version(),
        "params": params,
        "idempotencyKey": null,
    })
}
