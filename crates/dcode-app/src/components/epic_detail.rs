use dcode_api::EpicSummary;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::*;

#[derive(IntoElement)]
pub(crate) struct EpicDetail {
    epic: EpicSummary,
}

impl EpicDetail {
    pub(crate) fn new(epic: EpicSummary) -> Self {
        Self { epic }
    }
}

impl RenderOnce for EpicDetail {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        let counts = artifact_counts(&self.epic);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.epic.title),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(colors.muted_foreground)
                    .child(counts),
            )
            .child(div().text_sm().child(self.epic.initial_user_prompt))
    }
}

fn artifact_counts(epic: &EpicSummary) -> String {
    format!(
        "{} specs · {} tickets · {} stories · {} reviews",
        epic.spec_count, epic.ticket_count, epic.story_count, epic.review_count
    )
}
