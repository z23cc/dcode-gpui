use dcode_api::{EpicSummary, HostEndpoint, HostStatus};
use gpui_kit::*;

const EPIC_PAGE_SIZE: u32 = 100;

pub(crate) enum Load<T> {
    Loading,
    Ready(T),
    Failed(SharedString),
}

pub(crate) struct HomeData {
    pub(crate) status: HostStatus,
    pub(crate) epics: Vec<EpicSummary>,
}

pub(crate) struct HomeState {
    pub(crate) load: Load<HomeData>,
    selected_epic_id: Option<String>,
    _fetch: Option<Task<()>>,
}

impl HomeState {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let mut state = Self {
            load: Load::Loading,
            selected_epic_id: None,
            _fetch: None,
        };
        state.refresh(cx);
        state
    }

    pub(crate) fn refresh(&mut self, cx: &mut Context<Self>) {
        self.load = Load::Loading;
        cx.notify();
        let fetch = cx.background_spawn(fetch_home());
        self._fetch = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |state, cx| {
                match result {
                    Ok(data) => state.show(data),
                    Err(error) => state.load = Load::Failed(format!("{error:#}").into()),
                }
                cx.notify();
            })
            .ok();
        }));
    }

    pub(crate) fn select(&mut self, epic_id: &str, cx: &mut Context<Self>) {
        self.selected_epic_id = Some(epic_id.to_owned());
        cx.notify();
    }

    pub(crate) fn epics(&self) -> &[EpicSummary] {
        match &self.load {
            Load::Ready(data) => &data.epics,
            Load::Loading | Load::Failed(_) => &[],
        }
    }

    pub(crate) fn selected_epic(&self) -> Option<&EpicSummary> {
        let selected = self.selected_epic_id.as_deref()?;
        self.epics().iter().find(|epic| epic.id == selected)
    }

    fn show(&mut self, data: HomeData) {
        let still_listed = data
            .epics
            .iter()
            .any(|epic| Some(epic.id.as_str()) == self.selected_epic_id.as_deref());
        if !still_listed {
            self.selected_epic_id = data.epics.first().map(|epic| epic.id.clone());
        }
        self.load = Load::Ready(data);
    }
}

async fn fetch_home() -> anyhow::Result<HomeData> {
    let host = HostEndpoint::discover()?;
    let status = dcode_api::host_status(&host).await?;
    let epics = dcode_api::list_epics(&host, EPIC_PAGE_SIZE).await?;
    Ok(HomeData { status, epics })
}

#[cfg(test)]
mod tests {
    use dcode_api::{EpicSummary, HostStatus};

    use super::{HomeData, HomeState, Load};

    fn epic(id: &str) -> EpicSummary {
        EpicSummary {
            id: id.to_owned(),
            title: format!("title {id}"),
            initial_user_prompt: String::new(),
            spec_count: 0,
            ticket_count: 0,
            story_count: 0,
            review_count: 0,
        }
    }

    fn data(ids: &[&str]) -> HomeData {
        HomeData {
            status: HostStatus {
                host_version: "1".to_owned(),
                busy: false,
            },
            epics: ids.iter().map(|id| epic(id)).collect(),
        }
    }

    fn state_selecting(id: Option<&str>, listed: &[&str]) -> HomeState {
        let mut state = HomeState {
            load: Load::Loading,
            selected_epic_id: id.map(str::to_owned),
            _fetch: None,
        };
        state.show(data(listed));
        state
    }

    fn selected_id(state: &HomeState) -> Option<&str> {
        state.selected_epic().map(|epic| epic.id.as_str())
    }

    #[test]
    fn first_load_selects_the_first_epic() {
        assert_eq!(selected_id(&state_selecting(None, &["a", "b"])), Some("a"));
    }

    #[test]
    fn selection_survives_a_refresh_that_reorders_the_list() {
        let mut state = state_selecting(Some("b"), &["a", "b", "c"]);
        state.show(data(&["c", "b", "a"]));
        assert_eq!(selected_id(&state), Some("b"));
        assert_eq!(state.epics()[0].id, "c");
    }

    #[test]
    fn selection_falls_back_to_the_first_epic_when_the_selected_one_disappears() {
        let mut state = state_selecting(Some("b"), &["a", "b"]);
        state.show(data(&["c", "a"]));
        assert_eq!(selected_id(&state), Some("c"));
    }

    #[test]
    fn an_empty_list_selects_nothing() {
        let mut state = state_selecting(Some("a"), &["a"]);
        state.show(data(&[]));
        assert!(state.selected_epic().is_none());
        assert!(state.epics().is_empty());
    }

    #[test]
    fn selected_epic_follows_the_id_not_the_old_index() {
        let mut state = state_selecting(Some("a"), &["a", "b"]);
        state.show(data(&["b", "a"]));
        assert_eq!(selected_id(&state), Some("a"));
    }
}
