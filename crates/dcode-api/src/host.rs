use crate::{
    HostEndpoint, HostStatus, Result,
    wire::{self, NoParams},
};

pub async fn host_status(host: &HostEndpoint) -> Result<HostStatus> {
    wire::call(host, "host.status", &NoParams {}).await
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::HostStatus;

    #[test]
    fn host_status_ignores_fields_it_does_not_model() {
        let status: HostStatus = serde_json::from_value(json!({
            "hostVersion": "1.2.3",
            "busy": true,
            "pid": 4242,
            "startedAt": "2026-01-01T00:00:00Z",
        }))
        .unwrap();
        assert_eq!(status.host_version, "1.2.3");
        assert!(status.busy);
    }
}
