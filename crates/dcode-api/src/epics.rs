use crate::{
    EpicSummary, HostEndpoint, Result,
    wire::{self, ListTasksParams, ListTasksResponse},
};

pub async fn list_epics(host: &HostEndpoint, limit: u32) -> Result<Vec<EpicSummary>> {
    let params = ListTasksParams::most_recent(limit);
    let response: ListTasksResponse = wire::call(host, "epic.listTasks", &params).await?;
    Ok(summaries(response))
}

fn summaries(response: ListTasksResponse) -> Vec<EpicSummary> {
    response
        .tasks
        .into_iter()
        .filter_map(|task| task.epic?.light)
        .collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ListTasksResponse, summaries};

    #[test]
    fn summaries_skip_rows_without_an_epic_or_light_summary() {
        let light = |id: &str| {
            json!({
                "id": id,
                "title": "T",
                "initialUserPrompt": "P",
                "specCount": 1,
                "ticketCount": 2,
                "storyCount": 3,
                "reviewCount": 4,
                "unknownField": true,
            })
        };
        let response: ListTasksResponse = serde_json::from_value(json!({
            "tasks": [
                { "epic": { "light": light("a") } },
                { "epic": null },
                { "epic": { "light": null } },
                { "epic": { "light": light("b") }, "extra": 1 },
            ]
        }))
        .unwrap();
        let summaries = summaries(response);
        let ids: Vec<_> = summaries.iter().map(|epic| epic.id.as_str()).collect();
        assert_eq!(ids, ["a", "b"]);
        assert_eq!(summaries[0].ticket_count, 2);
    }
}
