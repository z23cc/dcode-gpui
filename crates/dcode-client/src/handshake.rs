use std::sync::LazyLock;

use serde_json::{Value, json};

const CLIENT_KIND: &str = "gpui";
const CLIENT_COMPATIBILITY_EPOCH: u32 = 2;

#[expect(
    clippy::expect_used,
    reason = "manifest.json is embedded at build time and a test parses it"
)]
static HANDSHAKE_MANIFESTS: LazyLock<Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("../manifest.json")).expect("manifest.json is valid JSON")
});

pub(crate) fn rpc_open_frame() -> Value {
    json!({
        "kind": "open",
        "manifest": HANDSHAKE_MANIFESTS["rpc"]["manifest"],
        "optionalManifest": HANDSHAKE_MANIFESTS["rpc"]["optionalManifest"],
        "clientIdentity": client_identity(),
    })
}

pub(crate) fn stream_open_frame() -> Value {
    json!({
        "kind": "open",
        "manifest": HANDSHAKE_MANIFESTS["stream"],
        "clientIdentity": client_identity(),
    })
}

pub(crate) fn host_serves(open_ack: &Value, method: &str) -> bool {
    ["manifest", "optionalManifest"]
        .iter()
        .any(|manifest| open_ack[manifest].get(method).is_some())
}

pub(crate) fn schema_version() -> Value {
    json!({ "major": 1, "minor": 0 })
}

fn client_identity() -> Value {
    json!({
        "kind": CLIENT_KIND,
        "compatibilityEpoch": CLIENT_COMPATIBILITY_EPOCH,
        "appVersion": env!("CARGO_PKG_VERSION"),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{HANDSHAKE_MANIFESTS, rpc_open_frame, stream_open_frame};

    #[test]
    fn embedded_manifest_holds_the_floor_methods() {
        let rpc = rpc_open_frame();
        assert!(rpc["manifest"]["host.status"].is_object());
        assert!(rpc["optionalManifest"].is_object());
        assert!(stream_open_frame()["manifest"]["agent.activity.subscribe"].is_object());
    }

    #[test]
    fn every_embedded_method_is_version_one_point_zero() {
        let manifests = [
            &HANDSHAKE_MANIFESTS["rpc"]["manifest"],
            &HANDSHAKE_MANIFESTS["rpc"]["optionalManifest"],
            &HANDSHAKE_MANIFESTS["stream"],
        ];
        for manifest in manifests {
            let methods = manifest.as_object().unwrap();
            assert!(!methods.is_empty());
            for (method, version) in methods {
                assert_eq!(version["major"], 1, "{method}");
                assert_eq!(version["minor"], 0, "{method}");
                assert_eq!(version["supportedMajors"], json!([1]), "{method}");
            }
        }
    }
}
