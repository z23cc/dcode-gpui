use std::{future::Future, time::Duration};

use anyhow::{Result, anyhow, bail};
use async_net::{TcpListener, TcpStream};
use async_tungstenite::{
    WebSocketStream, accept_hdr_async,
    tungstenite::{
        Message,
        handshake::server::{ErrorResponse, Request, Response},
    },
};
use dcode_client::{Error, HostEndpoint, call, subscribe};
use futures_util::StreamExt;
use serde_json::{Value, json};

const SECRET_PAYLOAD: &[u8] = b"SECRET-PAYLOAD-BYTES";

type Ws = WebSocketStream<TcpStream>;

const BINARY_PAYLOAD_ABOVE_DEFAULT_FRAME_CAP: usize = 17 << 20;
const CLIENT_GIVES_UP_AFTER: Duration = Duration::from_millis(50);
const HOST_SENDS_PAYLOAD_AFTER: Duration = Duration::from_millis(300);

#[expect(
    clippy::result_large_err,
    reason = "the accept_hdr_async callback must return tungstenite's ErrorResponse"
)]
async fn fake_host<T, F, Fut>(serve: F) -> Result<(HostEndpoint, smol::Task<Result<T>>)>
where
    T: Send + 'static,
    F: FnOnce(Ws, String) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T>> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let host = HostEndpoint {
        host_id: "test".into(),
        websocket_url: format!("ws://{}/rpc", listener.local_addr()?),
    };
    let task = smol::spawn(async move {
        let (tcp, _) = listener.accept().await?;
        let mut path = String::new();
        let ws = accept_hdr_async(
            tcp,
            |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
                request.uri().path().clone_into(&mut path);
                Ok(response)
            },
        )
        .await?;
        serve(ws, path).await
    });
    Ok((host, task))
}

async fn recv(ws: &mut Ws) -> Result<Value> {
    match ws.next().await.ok_or_else(|| anyhow!("client hung up"))?? {
        Message::Text(text) => Ok(serde_json::from_str(text.as_str())?),
        other => bail!("expected text, got {other:?}"),
    }
}

async fn put(ws: &mut Ws, frame: Value) -> Result<()> {
    ws.send(Message::text(frame.to_string())).await?;
    Ok(())
}

fn open_ack(method: &str) -> Value {
    json!({ "kind": "openAck", "manifest": { method: {} }, "optionalManifest": {} })
}

#[test]
fn call_returns_result() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, path| async move {
            let open = recv(&mut ws).await?;
            put(&mut ws, open_ack("host.status")).await?;
            let request = recv(&mut ws).await?;
            let response = json!({ "kind": "response", "result": { "ok": 1 } });
            put(&mut ws, response).await?;
            Ok((path, open, request))
        })
        .await
        .unwrap();

        let result = call(&host, "host.status", json!({ "a": 1 })).await.unwrap();
        assert_eq!(result, json!({ "ok": 1 }));

        let (path, open, request) = server.await.unwrap();
        assert_eq!(path, "/rpc");
        assert_eq!(open["kind"], "open");
        assert!(!open["manifest"].as_object().unwrap().is_empty());
        assert_eq!(open["clientIdentity"]["compatibilityEpoch"], 2);
        assert_eq!(request["kind"], "request");
        assert_eq!(request["method"], "host.status");
        assert_eq!(request["schemaVersion"], json!({ "major": 1, "minor": 0 }));
        assert_eq!(request["params"], json!({ "a": 1 }));
    });
}

#[test]
fn call_surfaces_response_error() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, _| async move {
            recv(&mut ws).await?;
            put(&mut ws, open_ack("host.status")).await?;
            recv(&mut ws).await?;
            let error = json!({ "code": "BOOM", "message": "it broke" });
            put(&mut ws, json!({ "kind": "response", "error": error })).await
        })
        .await
        .unwrap();

        let err = call(&host, "host.status", json!({})).await.unwrap_err();
        server.await.unwrap();

        let Error::Rpc {
            method,
            code,
            message,
        } = err
        else {
            panic!("expected Error::Rpc, got {err:?}");
        };
        assert_eq!(method, "host.status");
        assert_eq!(code, "BOOM");
        assert_eq!(message, "it broke");
    });
}

#[test]
fn call_surfaces_fatal_error_on_open() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, _| async move {
            recv(&mut ws).await?;
            let details = json!({
                "code": "INCOMPATIBLE",
                "reason": "manifest mismatch",
                "incompatibleMethods": [{ "method": "host.status", "blocking": "major" }],
            });
            put(&mut ws, json!({ "kind": "fatalError", "details": details })).await
        })
        .await
        .unwrap();

        let err = call(&host, "host.status", json!({})).await.unwrap_err();
        server.await.unwrap();

        let Error::Refused {
            code,
            reason,
            incompatible_methods,
        } = err
        else {
            panic!("expected Error::Refused, got {err:?}");
        };
        assert_eq!(code, "INCOMPATIBLE");
        assert_eq!(reason, "manifest mismatch");
        let methods = incompatible_methods.unwrap();
        assert_eq!(methods.len(), 1);
        assert_eq!(methods[0].method, "host.status");
    });
}

#[test]
fn call_rejects_a_method_the_host_does_not_serve() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, _| async move {
            recv(&mut ws).await?;
            put(&mut ws, open_ack("host.other")).await
        })
        .await
        .unwrap();

        let err = call(&host, "host.status", json!({})).await.unwrap_err();
        server.await.unwrap();

        assert!(matches!(err, Error::Unsupported(method) if method == "host.status"));
    });
}

#[test]
fn subscription_pongs_and_pairs_binary() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, path| async move {
            let open = recv(&mut ws).await?;
            put(&mut ws, open_ack("terminal.output")).await?;
            let subscribe = recv(&mut ws).await?;
            put(&mut ws, json!({ "kind": "ping" })).await?;
            put(&mut ws, json!({ "kind": "data", "hasBinaryPayload": true })).await?;
            ws.send(Message::binary(vec![1u8, 2, 3])).await?;
            let pong = recv(&mut ws).await?;
            ws.close(None).await?;
            Ok((path, open, subscribe, pong))
        })
        .await
        .unwrap();

        let mut sub = subscribe(&host, "terminal.output", json!({ "id": 7 }))
            .await
            .unwrap();
        let frame = sub.next().await.unwrap().unwrap();
        assert_eq!(frame.frame["kind"], "data");
        assert_eq!(frame.binary, Some(vec![1, 2, 3]));
        assert!(sub.next().await.unwrap().is_none());

        let (path, open, subscribe, pong) = server.await.unwrap();
        assert_eq!(path, "/stream");
        assert_eq!(open["kind"], "open");
        assert_eq!(subscribe["kind"], "subscribe");
        assert_eq!(subscribe["method"], "terminal.output");
        assert_eq!(subscribe["params"], json!({ "id": 7 }));
        assert_eq!(pong, json!({ "kind": "pong", "hasBinaryPayload": false }));
    });
}

#[test]
fn subscription_next_is_cancel_safe_between_envelope_and_payload() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, _| async move {
            recv(&mut ws).await?;
            put(&mut ws, open_ack("terminal.output")).await?;
            recv(&mut ws).await?;
            put(&mut ws, json!({ "kind": "data", "hasBinaryPayload": true })).await?;
            smol::Timer::after(HOST_SENDS_PAYLOAD_AFTER).await;
            ws.send(Message::binary(vec![9u8, 8, 7])).await?;
            ws.close(None).await?;
            Ok(())
        })
        .await
        .unwrap();

        let mut sub = subscribe(&host, "terminal.output", json!({}))
            .await
            .unwrap();
        let first = smol::future::or(async { Some(sub.next().await) }, async {
            smol::Timer::after(CLIENT_GIVES_UP_AFTER).await;
            None
        })
        .await;
        assert!(first.is_none());

        let frame = sub.next().await.unwrap().unwrap();
        assert_eq!(frame.frame["kind"], "data");
        assert_eq!(frame.binary, Some(vec![9, 8, 7]));
        server.await.unwrap();
    });
}

#[test]
fn subscription_receives_a_payload_above_the_default_frame_cap() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, _| async move {
            recv(&mut ws).await?;
            put(&mut ws, open_ack("terminal.output")).await?;
            recv(&mut ws).await?;
            put(&mut ws, json!({ "kind": "data", "hasBinaryPayload": true })).await?;
            let payload = vec![7u8; BINARY_PAYLOAD_ABOVE_DEFAULT_FRAME_CAP];
            ws.send(Message::binary(payload)).await?;
            ws.close(None).await?;
            Ok(())
        })
        .await
        .unwrap();

        let mut sub = subscribe(&host, "terminal.output", json!({}))
            .await
            .unwrap();
        let frame = sub.next().await.unwrap().unwrap();
        server.await.unwrap();

        assert_eq!(
            frame.binary.map(|bytes| bytes.len()),
            Some(BINARY_PAYLOAD_ABOVE_DEFAULT_FRAME_CAP)
        );
    });
}

#[test]
fn unexpected_binary_frame_is_described_without_its_bytes() {
    smol::block_on(async {
        let (host, server) = fake_host(|mut ws, _| async move {
            recv(&mut ws).await?;
            ws.send(Message::binary(SECRET_PAYLOAD.to_vec())).await?;
            Ok(())
        })
        .await
        .unwrap();

        let err = call(&host, "host.status", json!({})).await.unwrap_err();
        server.await.unwrap();

        let Error::UnexpectedFrame(description) = &err else {
            panic!("expected Error::UnexpectedFrame, got {err:?}");
        };
        assert!(description.contains(&SECRET_PAYLOAD.len().to_string()));
        assert!(!description.contains("SECRET"), "{description}");
        assert!(!format!("{err:?}").contains("SECRET"));
    });
}
