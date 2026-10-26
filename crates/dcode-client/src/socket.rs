use std::time::Duration;

use async_net::TcpStream;
use async_tungstenite::{
    WebSocketStream, client_async_with_config,
    tungstenite::{Message, protocol::WebSocketConfig},
};
use futures_util::StreamExt;
use serde_json::Value;

use crate::{Error, Result, error::FatalDetails};

pub(crate) type Socket = WebSocketStream<TcpStream>;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);
const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const HOST_MAX_MESSAGE_BYTES: usize = 1 << 30;

pub(crate) async fn open(url: &str, open_frame: Value) -> Result<(Socket, Value)> {
    within(HANDSHAKE_TIMEOUT, "openAck", async {
        let mut socket = dial(url).await?;
        send(&mut socket, &open_frame).await?;
        let ack = expect(&mut socket, "openAck").await?;
        Ok((socket, ack))
    })
    .await
}

async fn dial(url: &str) -> Result<Socket> {
    let authority = url
        .strip_prefix("ws://")
        .and_then(|rest| rest.split('/').next())
        .ok_or_else(|| Error::NotWebSocketUrl(url.to_owned()))?;
    let tcp = TcpStream::connect(authority)
        .await
        .map_err(|source| Error::Unreachable {
            url: url.to_owned(),
            source,
        })?;
    let config = WebSocketConfig::default()
        .max_frame_size(None)
        .max_message_size(Some(HOST_MAX_MESSAGE_BYTES));
    let (socket, _) = client_async_with_config(url, tcp, Some(config)).await?;
    Ok(socket)
}

pub(crate) async fn send(socket: &mut Socket, frame: &Value) -> Result<()> {
    within(WRITE_TIMEOUT, "send", async {
        socket.send(Message::text(frame.to_string())).await?;
        Ok(())
    })
    .await
}

pub(crate) async fn close(socket: &mut Socket) {
    let _ = within(WRITE_TIMEOUT, "close", async {
        socket.close(None).await?;
        Ok(())
    })
    .await;
}

pub(crate) fn describe(message: Option<&Message>) -> String {
    match message {
        None => "the connection closing".to_owned(),
        Some(Message::Text(text)) => format!("a {}-byte text frame", text.len()),
        Some(Message::Binary(bytes)) => format!("a {}-byte binary frame", bytes.len()),
        Some(other) => format!("a {} frame", control_frame_name(other)),
    }
}

fn control_frame_name(message: &Message) -> &'static str {
    match message {
        Message::Ping(_) => "ping",
        Message::Pong(_) => "pong",
        Message::Close(_) => "close",
        Message::Text(_) | Message::Binary(_) | Message::Frame(_) => "raw",
    }
}

pub(crate) async fn recv(socket: &mut Socket) -> Result<Option<Message>> {
    while let Some(message) = socket.next().await {
        match message? {
            message @ (Message::Text(_) | Message::Binary(_)) => return Ok(Some(message)),
            Message::Close(_) => return Ok(None),
            Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
        }
    }
    Ok(None)
}

pub(crate) async fn recv_json(socket: &mut Socket) -> Result<Option<Value>> {
    match recv(socket).await? {
        None => Ok(None),
        Some(Message::Text(text)) => {
            let mut frame: Value = serde_json::from_str(text.as_str())?;
            if frame["kind"] == "fatalError" {
                let details: FatalDetails = serde_json::from_value(frame["details"].take())?;
                return Err(details.into());
            }
            Ok(Some(frame))
        }
        other => Err(Error::UnexpectedFrame(format!(
            "expected a text frame, got {}",
            describe(other.as_ref())
        ))),
    }
}

pub(crate) async fn expect(socket: &mut Socket, kind: &'static str) -> Result<Value> {
    let frame = recv_json(socket).await?.ok_or(Error::Closed(kind))?;
    if frame["kind"] == kind {
        Ok(frame)
    } else {
        Err(Error::UnexpectedFrame(format!(
            "expected {kind}, got {}",
            frame["kind"].as_str().unwrap_or("a frame without a kind")
        )))
    }
}

pub(crate) async fn within<T>(
    limit: Duration,
    waiting_for: &'static str,
    work: impl Future<Output = Result<T>>,
) -> Result<T> {
    futures_lite::future::or(work, async move {
        async_io::Timer::after(limit).await;
        Err(Error::Timeout { waiting_for, limit })
    })
    .await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_tungstenite::tungstenite::Message;

    use super::{describe, within};
    use crate::{Error, Result};

    #[test]
    fn describe_reports_a_binary_frame_length_without_its_bytes() {
        let description = describe(Some(&Message::binary(vec![0xAB_u8; 1 << 20])));
        assert!(description.contains("1048576"), "{description}");
        assert!(description.len() < 80, "{description}");
    }

    #[test]
    fn within_times_out_a_stalled_wait() {
        let limit = Duration::from_millis(20);
        let result: Result<()> = futures_lite::future::block_on(within(limit, "stall", async {
            futures_lite::future::pending::<()>().await;
            Ok(())
        }));
        assert!(matches!(
            result,
            Err(Error::Timeout { waiting_for: "stall", limit: got }) if got == limit
        ));
    }
}
