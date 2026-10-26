use gpui_kit::*;

use crate::views::HomeView;

const MAIN_WINDOW_TITLE: &str = "DCode";
const MAIN_WINDOW_SIZE: Size<Pixels> = Size {
    width: px(1100.),
    height: px(720.),
};

pub(crate) fn run() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        quit_when_last_window_closes(cx);
        open_main_window(cx);
        cx.activate(true);
    });
}

fn quit_when_last_window_closes(cx: &mut App) {
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
}

#[expect(
    clippy::expect_used,
    reason = "the app cannot run without its main window"
)]
fn open_main_window(cx: &mut App) {
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(MAIN_WINDOW_TITLE.into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            MAIN_WINDOW_SIZE,
            cx,
        ))),
        ..Default::default()
    };
    open_window(options, cx, |_, cx| cx.new(HomeView::new)).expect("main window opens");
}
