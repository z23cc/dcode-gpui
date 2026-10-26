use std::{fmt::Write as _, path::PathBuf, time::Duration};

use serde::Deserialize;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HOME is not set")]
    NoHome,
    #[error("no running host: cannot read {}", .path.display())]
    NoHost {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("malformed {}", .path.display())]
    MalformedPidFile {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("not a ws:// url: {0}")]
    NotWebSocketUrl(String),
    #[error("cannot reach the host at {url}")]
    Unreachable { url: String, source: std::io::Error },
    #[error("WebSocket failure")]
    WebSocket(#[from] async_tungstenite::tungstenite::Error),
    #[error("no {waiting_for} from the host within {} s", .limit.as_secs())]
    Timeout {
        waiting_for: &'static str,
        limit: Duration,
    },
    #[error("the host closed the connection before {0}")]
    Closed(&'static str),
    #[error("host refused: {code} {reason}{}", list(.incompatible_methods.as_deref()))]
    Refused {
        code: String,
        reason: String,
        incompatible_methods: Option<Vec<IncompatibleMethod>>,
    },
    #[error("host does not serve {0}")]
    Unsupported(String),
    #[error("{method} failed: {code} {message}")]
    Rpc {
        method: String,
        code: String,
        message: String,
    },
    #[error("unexpected frame from the host: {0}")]
    UnexpectedFrame(String),
    #[error("malformed frame from the host")]
    MalformedFrame(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct IncompatibleMethod {
    pub method: String,
    pub blocking: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FatalDetails {
    code: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    incompatible_methods: Option<Vec<IncompatibleMethod>>,
}

impl From<FatalDetails> for Error {
    fn from(details: FatalDetails) -> Self {
        Self::Refused {
            code: details.code,
            reason: details.reason,
            incompatible_methods: details.incompatible_methods,
        }
    }
}

fn list(methods: Option<&[IncompatibleMethod]>) -> String {
    methods
        .into_iter()
        .flatten()
        .fold(String::new(), |mut out, method| {
            let _ = write!(out, "\n  {} ({})", method.method, method.blocking);
            out
        })
}

#[cfg(test)]
mod tests {
    use super::{Error, IncompatibleMethod};

    #[test]
    fn refused_lists_each_incompatible_method() {
        let methods = ["host.status", "epic.listTasks"].map(|method| IncompatibleMethod {
            method: method.into(),
            blocking: "major".into(),
        });
        let message = Error::Refused {
            code: "INCOMPATIBLE".into(),
            reason: "manifest mismatch".into(),
            incompatible_methods: Some(methods.to_vec()),
        }
        .to_string();
        assert_eq!(
            message,
            "host refused: INCOMPATIBLE manifest mismatch\n  host.status (major)\n  epic.listTasks (major)"
        );
    }
}
