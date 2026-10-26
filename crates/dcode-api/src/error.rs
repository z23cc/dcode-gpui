pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Client(#[from] dcode_client::Error),
    #[error("cannot encode {method} params")]
    Encode {
        method: &'static str,
        source: serde_json::Error,
    },
    #[error("unexpected {method} result")]
    Decode {
        method: &'static str,
        source: serde_json::Error,
    },
}
