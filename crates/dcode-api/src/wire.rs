use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

use crate::{Error, HostEndpoint, Result};

const TS_GUI_LIST_TASKS_EPIC_EXTENSION_VERSION: &str = "2.0.0";
const TS_GUI_LIST_TASKS_PHASE_EXTENSION_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStatus {
    pub host_version: String,
    pub busy: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpicSummary {
    pub id: String,
    pub title: String,
    pub initial_user_prompt: String,
    pub spec_count: u32,
    pub ticket_count: u32,
    pub story_count: u32,
    pub review_count: u32,
}

#[derive(Serialize)]
pub(crate) struct NoParams {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ListTasksParams {
    limit: u32,
    filters: Option<Value>,
    sort: &'static str,
    extension_phase_version: &'static str,
    extension_epic_version: &'static str,
}

impl ListTasksParams {
    pub(crate) fn most_recent(limit: u32) -> Self {
        Self {
            limit,
            filters: None,
            sort: "recent",
            extension_phase_version: TS_GUI_LIST_TASKS_PHASE_EXTENSION_VERSION,
            extension_epic_version: TS_GUI_LIST_TASKS_EPIC_EXTENSION_VERSION,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct ListTasksResponse {
    pub(crate) tasks: Vec<TaskRow>,
}

#[derive(Deserialize)]
pub(crate) struct TaskRow {
    pub(crate) epic: Option<EpicRow>,
}

#[derive(Deserialize)]
pub(crate) struct EpicRow {
    pub(crate) light: Option<EpicSummary>,
}

pub(crate) async fn call<P: Serialize, T: DeserializeOwned>(
    host: &HostEndpoint,
    method: &'static str,
    params: &P,
) -> Result<T> {
    let params = serde_json::to_value(params).map_err(|source| Error::Encode { method, source })?;
    let result = dcode_client::call(host, method, params).await?;
    serde_json::from_value(result).map_err(|source| Error::Decode { method, source })
}

#[cfg(test)]
mod tests {
    use super::{ListTasksParams, NoParams};

    #[test]
    fn list_tasks_params_serialize_to_the_host_shape() {
        assert_eq!(
            serde_json::to_string(&ListTasksParams::most_recent(20)).unwrap(),
            r#"{"limit":20,"filters":null,"sort":"recent","extensionPhaseVersion":"1.0.0","extensionEpicVersion":"2.0.0"}"#
        );
    }

    #[test]
    fn no_params_serialize_to_an_empty_object() {
        assert_eq!(serde_json::to_string(&NoParams {}).unwrap(), "{}");
    }
}
