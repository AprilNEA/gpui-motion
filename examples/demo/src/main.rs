use gpui::{
    App, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, rgba, size,
};
use gpui_motion::{MotionExt, MotionValue, Spring, presence};
use gpui_platform::application;

struct MotionDemo {
    panel_open: bool,
    toast_visible: bool,
    progress_step: usize,
    progress: MotionValue<gpui::Pixels>,
}

impl Render for MotionDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel_target = if self.panel_open {
            (px(284.0), rgb(0x7c5cff))
        } else {
            (px(16.0), rgb(0x24b47e))
        };
        let progress = self.progress.get(window, cx);

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_4()
            .p_6()
            .bg(rgb(0x11131a))
            .text_color(rgb(0xf4f5f8))
            .child(
                div()
                    .text_2xl()
                    .child("gpui-motion · state-driven animation"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x1c202b))
                    .child("MotionElement — change the target rapidly to redirect the spring")
                    .child(
                        div()
                            .id("toggle-panel")
                            .w(px(150.0))
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(rgb(0x303747))
                            .hover(|style| style.bg(rgb(0x3d465a)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.panel_open = !this.panel_open;
                                cx.notify();
                            }))
                            .child(if self.panel_open {
                                "Move panel back"
                            } else {
                                "Move panel"
                            }),
                    )
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(px(64.0))
                            .rounded_md()
                            .bg(rgb(0x151821))
                            .child(
                                div()
                                    .absolute()
                                    .top(px(12.0))
                                    .w(px(96.0))
                                    .h(px(40.0))
                                    .rounded_md()
                                    .with_motion(
                                        "spring-panel",
                                        panel_target,
                                        Spring::wobbly().into(),
                                        |panel, (left, color)| panel.left(left).bg(color),
                                    )
                                    .initial((px(16.0), rgb(0x24b47e))),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x1c202b))
                    .child("presence — exit remains mounted until its animation settles")
                    .child(
                        div()
                            .id("toggle-toast")
                            .w(px(150.0))
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(rgb(0x303747))
                            .hover(|style| style.bg(rgb(0x3d465a)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toast_visible = !this.toast_visible;
                                cx.notify();
                            }))
                            .child(if self.toast_visible {
                                "Hide toast"
                            } else {
                                "Show toast"
                            }),
                    )
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .h(px(72.0))
                            .rounded_md()
                            .bg(rgb(0x151821))
                            .child(presence(
                                "demo-toast",
                                self.toast_visible,
                                (px(180.0), rgba(0xffb020ff)),
                                (px(390.0), rgba(0xffb02000)),
                                Spring::stiff().into(),
                                |(left, color)| {
                                    div()
                                        .absolute()
                                        .left(left)
                                        .top(px(14.0))
                                        .w(px(200.0))
                                        .h(px(44.0))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .bg(color)
                                        .text_color(rgb(0x17120a))
                                        .child("Saved successfully")
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .rounded_lg()
                    .bg(rgb(0x1c202b))
                    .child(format!(
                        "MotionValue<Pixels> — current width {:.0}px",
                        f32::from(progress)
                    ))
                    .child(
                        div()
                            .id("advance-progress")
                            .w(px(180.0))
                            .px_3()
                            .py_2()
                            .rounded_md()
                            .cursor_pointer()
                            .bg(rgb(0x303747))
                            .hover(|style| style.bg(rgb(0x3d465a)))
                            .on_click(cx.listener(|this, _, _, cx| {
                                const TARGETS: [f32; 4] = [64.0, 176.0, 320.0, 112.0];
                                this.progress_step = (this.progress_step + 1) % TARGETS.len();
                                this.progress.set_target(px(TARGETS[this.progress_step]));
                                cx.notify();
                            }))
                            .child("Next progress target"),
                    )
                    .child(
                        div()
                            .w(px(360.0))
                            .h(px(18.0))
                            .rounded_lg()
                            .bg(rgb(0x151821))
                            .child(div().w(progress).h_full().rounded_lg().bg(rgb(0x4cc9f0))),
                    ),
            )
    }
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(760.0), px(700.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| MotionDemo {
                    panel_open: false,
                    toast_visible: false,
                    progress_step: 0,
                    progress: MotionValue::new(px(64.0), Spring::gentle().into()),
                })
            },
        )
        .expect("failed to open demo window");
        cx.activate(true);
    });
}
