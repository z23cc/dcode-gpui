#![expect(clippy::print_stdout, reason = "the probe reports to the terminal")]

use std::time::Duration;

use anyhow::{Result, bail, ensure};
use dcode_client::{HostEndpoint, call, subscribe};
use serde_json::json;

const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(5);

fn main() -> Result<()> {
    smol::block_on(async {
        let host = HostEndpoint::discover()?;
        println!("host {} at {}", host.host_id, host.websocket_url);
        probe_status(&host).await?;
        probe_epics(&host).await?;
        probe_activity_stream(&host).await
    })
}

async fn probe_status(host: &HostEndpoint) -> Result<()> {
    let status = call(host, "host.status", json!({})).await?;
    ensure!(status.is_object(), "host.status returned {status}");
    println!("host.status ok: {}", truncate(&status.to_string(), 160));
    Ok(())
}

async fn probe_epics(host: &HostEndpoint) -> Result<()> {
    let params = json!({
        "limit": 20,
        "filters": null,
        "sort": "recent",
        "extensionPhaseVersion": "1.0.0",
        "extensionEpicVersion": "2.0.0",
    });
    let tasks = call(host, "epic.listTasks", params).await?;
    let tasks = tasks["tasks"].as_array().cloned().unwrap_or_default();
    println!("epic.listTasks: {} epics", tasks.len());
    for task in tasks.iter().take(5) {
        let title = task["epic"]["light"]["title"]
            .as_str()
            .unwrap_or("(untitled)");
        println!("  - {title}");
    }
    Ok(())
}

async fn probe_activity_stream(host: &HostEndpoint) -> Result<()> {
    let mut stream = subscribe(
        host,
        "agent.activity.subscribe",
        json!({ "plane": "local-only" }),
    )
    .await?;
    let first = smol::future::or(stream.next(), async {
        smol::Timer::after(FIRST_FRAME_TIMEOUT).await;
        Ok(None)
    })
    .await?;
    stream.close().await;
    let Some(first) = first else {
        bail!("agent.activity.subscribe closed or sent nothing in 5 s");
    };
    ensure!(
        first.frame["kind"] == "state",
        "expected a state frame, got {}",
        first.frame
    );
    println!(
        "agent.activity.subscribe state: {}",
        truncate(&first.frame.to_string(), 200)
    );
    Ok(())
}

fn truncate(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text.to_owned(),
    }
}
