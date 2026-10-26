use std::path::PathBuf;

use serde::Deserialize;

use crate::{Error, Result};

const PID_FILE_OVERRIDE_ENV: &str = "DCODE_PID_JSON";
const BETA_PID_FILE_IN_HOME: &str = ".dcode/host/staging/pid.json";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostEndpoint {
    pub host_id: String,
    pub websocket_url: String,
}

impl HostEndpoint {
    pub fn discover() -> Result<Self> {
        let path = pid_file_path()?;
        let text = std::fs::read_to_string(&path).map_err(|source| Error::NoHost {
            path: path.clone(),
            source,
        })?;
        serde_json::from_str(&text).map_err(|source| Error::MalformedPidFile { path, source })
    }

    pub(crate) fn stream_url(&self) -> String {
        match self.websocket_url.strip_suffix("/rpc") {
            Some(base) => format!("{base}/stream"),
            None => format!("{}/stream", self.websocket_url.trim_end_matches('/')),
        }
    }
}

fn pid_file_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(PID_FILE_OVERRIDE_ENV) {
        return Ok(PathBuf::from(path));
    }
    let home = std::env::var_os("HOME").ok_or(Error::NoHome)?;
    Ok(PathBuf::from(home).join(BETA_PID_FILE_IN_HOME))
}

#[cfg(test)]
mod tests {
    use super::HostEndpoint;

    fn endpoint(url: &str) -> HostEndpoint {
        HostEndpoint {
            host_id: "test".into(),
            websocket_url: url.into(),
        }
    }

    #[test]
    fn stream_url_replaces_the_rpc_suffix() {
        assert_eq!(
            endpoint("ws://127.0.0.1:4000/rpc").stream_url(),
            "ws://127.0.0.1:4000/stream"
        );
    }

    #[test]
    fn stream_url_appends_when_there_is_no_rpc_suffix() {
        assert_eq!(
            endpoint("ws://127.0.0.1:4000/").stream_url(),
            "ws://127.0.0.1:4000/stream"
        );
    }
}
