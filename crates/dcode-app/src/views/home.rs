use gpui_kit::component::{ActiveTheme as _, button::Button};
use gpui_kit::*;

use crate::components::{EpicDetail, SidebarItem};
use crate::state::{HomeState, Load};

const SIDEBAR_WIDTH: Rems = rems(16.);

pub(crate) struct HomeView {
    state: Entity<HomeState>,
    _subscriptions: Vec<Subscription>,
}

impl HomeView {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        let state = cx.new(HomeState::new);
        let subscriptions = vec![cx.observe(&state, |_, _, cx| cx.notify())];
        Self {
            state,
            _subscriptions: subscriptions,
        }
    }

    fn render_sidebar(&self, cx: &Context<Self>) -> Stateful<Div> {
        let colors = cx.theme().colors;
        let home = self.state.read(cx);
        let selected_id = home.selected_epic().map(|epic| epic.id.as_str());
        let items = home.epics().iter().map(|epic| {
            let state = self.state.clone();
            let epic_id = epic.id.clone();
            SidebarItem::new(SharedString::from(epic.id.clone()), epic.title.clone())
                .selected(Some(epic.id.as_str()) == selected_id)
                .on_click(move |_, _, cx| {
                    state.update(cx, |state, cx| state.select(&epic_id, cx));
                })
        });
        div()
            .id("epics")
            .w(SIDEBAR_WIDTH)
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .gap_0p5()
            .p_1()
            .overflow_y_scroll()
            .bg(colors.sidebar)
            .text_color(colors.sidebar_foreground)
            .border_r_1()
            .border_color(colors.sidebar_border)
            .child(
                div()
                    .px_2()
                    .py_1p5()
                    .text_xs()
                    .text_color(colors.muted_foreground)
                    .child("EPICS"),
            )
            .children(items)
    }

    fn render_header(&self, cx: &Context<Self>) -> Div {
        let colors = cx.theme().colors;
        let status = match &self.state.read(cx).load {
            Load::Ready(data) => {
                let activity = if data.status.busy { "busy" } else { "idle" };
                format!("host {} · {activity}", data.status.host_version)
            }
            Load::Loading | Load::Failed(_) => String::new(),
        };
        let state = self.state.clone();
        div()
            .flex()
            .items_center()
            .justify_between()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(status),
            )
            .child(
                Button::new("refresh")
                    .label("Refresh")
                    .on_click(move |_, _, cx| {
                        state.update(cx, HomeState::refresh);
                    }),
            )
    }

    fn render_body(&self, cx: &Context<Self>) -> Stateful<Div> {
        let colors = cx.theme().colors;
        let home = self.state.read(cx);
        let body = div().id("detail").flex_1().p_6().overflow_y_scroll();
        match (&home.load, home.selected_epic()) {
            (Load::Loading, _) => body
                .text_color(colors.muted_foreground)
                .child("Connecting to the host…"),
            (Load::Failed(error), _) => body.text_color(colors.danger).child(error.clone()),
            (Load::Ready(_), Some(epic)) => body.child(EpicDetail::new(epic.clone())),
            (Load::Ready(_), None) => body
                .text_color(colors.muted_foreground)
                .child("No epics on this host."),
        }
    }
}

impl Render for HomeView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors;
        div()
            .size_full()
            .flex()
            .bg(colors.background)
            .text_color(colors.foreground)
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(self.render_header(cx))
                    .child(self.render_body(cx)),
            )
    }
}
