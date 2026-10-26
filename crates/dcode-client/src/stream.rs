use std::time::Duration;

use async_tungstenite::tungstenite::Message;
use serde_json::{Value, json};

use crate::{
    Error, HostEndpoint, Result, handshake,
    socket::{Socket, close, describe, open, recv, recv_json, send, within},
};

const HOST_PING_INTERVAL: Duration = Duration::from_secs(25);
const HOST_PONG_DEADLINE: Duration = Duration::from_secs(60);
const SILENCE_LIMIT: Duration = HOST_PING_INTERVAL.saturating_add(HOST_PONG_DEADLINE);

#[derive(Debug)]
pub struct StreamFrame {
    pub frame: Value,
    pub binary: Option<Vec<u8>>,
}

pub struct HostStream {
    socket: Socket,
    envelope_awaiting_payload: Option<Value>,
}

pub async fn subscribe(host: &HostEndpoint, method: &str, params: Value) -> Result<HostStream> {
    let (mut socket, ack) = open(&host.stream_url(), handshake::stream_open_frame()).await?;
    if !handshake::host_serves(&ack, method) {
        return Err(Error::Unsupported(method.to_owned()));
    }
    let subscribe_frame = json!({
        "kind": "subscribe",
        "method": method,
        "schemaVersion": handshake::schema_version(),
        "params": params,
    });
    send(&mut socket, &subscribe_frame).await?;
    Ok(HostStream {
        socket,
        envelope_awaiting_payload: None,
    })
}

impl HostStream {
    pub async fn next(&mut self) -> Result<Option<StreamFrame>> {
        loop {
            if self.envelope_awaiting_payload.is_none() {
                let frame = within(SILENCE_LIMIT, "stream frame", recv_json(&mut self.socket));
                let Some(frame) = frame.await? else {
                    return Ok(None);
                };
                if frame["kind"] == "ping" {
                    send(&mut self.socket, &pong_frame()).await?;
                    continue;
                }
                if frame["hasBinaryPayload"] != true {
                    return Ok(Some(StreamFrame {
                        frame,
                        binary: None,
                    }));
                }
                self.envelope_awaiting_payload = Some(frame);
            }
            let payload = within(SILENCE_LIMIT, "binary payload", recv(&mut self.socket)).await?;
            let payload = match payload {
                Some(Message::Binary(bytes)) => bytes.to_vec(),
                other => {
                    return Err(Error::UnexpectedFrame(format!(
                        "expected a binary payload, got {}",
                        describe(other.as_ref())
                    )));
                }
            };
            if let Some(frame) = self.envelope_awaiting_payload.take() {
                return Ok(Some(StreamFrame {
                    frame,
                    binary: Some(payload),
                }));
            }
        }
    }

    pub async fn close(mut self) {
        close(&mut self.socket).await;
    }
}

fn pong_frame() -> Value {
    json!({ "kind": "pong", "hasBinaryPayload": false })
}
