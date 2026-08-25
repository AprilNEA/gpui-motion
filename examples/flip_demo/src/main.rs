use gpui::{
    App, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use gpui_motion::{FlipExt, Spring};
use gpui_platform::application;

struct FlipDemo {
    order: Vec<usize>,
}

impl Render for FlipDemo {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self
            .order
            .iter()
            .copied()
            .map(|id| {
                const COLORS: [u32; 8] = [
                    0x7c5cff, 0xff5c8a, 0x4cc9f0, 0xffb020, 0x24b47e, 0x9b5de5, 0xf15bb5, 0x00bbf9,
                ];

                div()
                    .w(px(136.0))
                    .h(px(88.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_lg()
                    .bg(rgb(COLORS[id]))
                    .text_color(rgb(0xffffff))
                    .child(format!("Item {}", id + 1))
                    .with_flip(("item", id))
                    .transition(Spring::stiff())
            })
            .collect::<Vec<_>>();

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_5()
            .p_8()
            .bg(rgb(0x11131a))
            .text_color(rgb(0xf4f5f8))
            .child(div().text_2xl().child("gpui-motion · FLIP layout"))
            .child(
                div()
                    .id("shuffle")
                    .px_5()
                    .py_2()
                    .rounded_lg()
                    .cursor_pointer()
                    .bg(rgb(0x303747))
                    .hover(|style| style.bg(rgb(0x3d465a)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.order.rotate_left(1);
                        this.order.swap(1, 6);
                        cx.notify();
                    }))
                    .child("Shuffle"),
            )
            .child(
                div()
                    .w(px(580.0))
                    .grid()
                    .grid_cols(4)
                    .gap_3()
                    .children(items),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(720.0), px(480.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| FlipDemo {
                    order: (0..8).collect(),
                })
            },
        )
        .expect("failed to open FLIP demo window");
        cx.activate(true);
    });
}
