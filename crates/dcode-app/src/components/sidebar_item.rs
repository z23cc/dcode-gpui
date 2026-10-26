use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct SidebarItem {
    id: ElementId,
    label: SharedString,
    selected: bool,
    on_click: Option<ClickHandler>,
}

impl SidebarItem {
    pub(crate) fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            on_click: None,
        }
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for SidebarItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = cx.theme().colors;
        div()
            .id(self.id)
            .px_2()
            .py_1p5()
            .rounded_md()
            .text_sm()
            .truncate()
            .cursor_pointer()
            .hover(move |row| row.bg(colors.list_hover))
            .when(self.selected, |row| row.bg(colors.list_active))
            .when_some(self.on_click, |row, handler| row.on_click(handler))
            .child(self.label)
            .test_support()
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::Cell, rc::Rc};

    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AppContext as _, Context, IntoElement, Render, TestAppContext, Window, base::Root,
    };

    use super::SidebarItem;

    struct Harness {
        clicks: Rc<Cell<u32>>,
    }

    impl Render for Harness {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            SidebarItem::new("item", "Epic").on_click(move |_, _, _| clicks.set(clicks.get() + 1))
        }
    }

    #[gpui_kit::test]
    fn clicking_the_item_runs_its_handler(cx: &mut TestAppContext) {
        let clicks = Rc::new(Cell::new(0));
        let handle = cx.update(|cx| {
            gpui_kit::init(cx);
            let (window, _) =
                gpui_kit::open_window(gpui_kit::WindowOptions::default(), cx, |_, cx| {
                    cx.new(|_| Harness {
                        clicks: clicks.clone(),
                    })
                })
                .unwrap();
            window.downcast::<Root>().unwrap()
        });

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("item", cx);
        })
        .unwrap();

        assert_eq!(clicks.get(), 1);
    }
}
