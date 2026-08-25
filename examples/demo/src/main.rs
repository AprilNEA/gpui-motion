use gpui::{
    App, Bounds, Context, ElementId, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    Render, Rgba, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, rgba, size,
};
use gpui_motion::{
    DragTracker, Inertia, KeyframesTiming, MotionExt, MotionValue, PresenceMode, RepeatKind,
    Spring, Transition, Tween, easing, presence, presence_group,
};
use gpui_platform::application;

struct MotionDemo {
    panel_open: bool,
    toast_visible: bool,
    progress_step: usize,
    progress: MotionValue<gpui::Pixels>,
    keyframes_forward: bool,
    per_property_active: bool,
    drag_x: MotionValue<gpui::Pixels>,
    drag_position: gpui::Pixels,
    drag: DragTracker,
    next_item: u64,
    sync_items: Vec<u64>,
    wait_first: bool,
}

impl MotionDemo {
    fn finish_drag(&mut self, cx: &mut Context<Self>) {
        if self.drag.is_dragging() {
            self.drag_x.flick(self.drag.end().x);
            cx.notify();
        }
    }
}

impl Render for MotionDemo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let panel_target = if self.panel_open {
            (px(284.0), rgb(0x7c5cff))
        } else {
            (px(16.0), rgb(0x24b47e))
        };
        let progress = self.progress.get(window, cx);
        let drag_x = self.drag_x.get(window, cx);
        if !self.drag.is_dragging() {
            self.drag_position = drag_x;
        }

        let keyframes = if self.keyframes_forward {
            [px(16.0), px(210.0), px(72.0), px(360.0)]
        } else {
            [px(360.0), px(72.0), px(210.0), px(16.0)]
        };
        let keyframes_timing = KeyframesTiming::new(1.4)
            .times(&[0.0, 0.25, 0.65, 1.0])
            .easings(&[easing::ease_out, easing::back_out, easing::circ_in]);

        let per_property_target = if self.per_property_active {
            (px(360.0), rgba(0xff5c8aff))
        } else {
            (px(24.0), rgba(0x4cc9f0ff))
        };

        let mut sync_group = presence_group::<(gpui::Pixels, Rgba)>("sync-list")
            .mode(PresenceMode::Sync)
            .enter((px(0.0), rgba(0x7c5cffff)))
            .exit((px(90.0), rgba(0x7c5cff00)))
            .transition(Spring::stiff());
        for (index, key) in self.sync_items.iter().copied().enumerate() {
            sync_group = sync_group.child(
                ElementId::named_usize("sync-item", key as usize),
                move |(offset, color)| {
                    div()
                        .relative()
                        .left(offset)
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(color)
                        .child(format!("Item {}", index + 1))
                        .into_any_element()
                },
            );
        }

        let wait_key = if self.wait_first { 1 } else { 2 };
        let wait_group = presence_group::<(gpui::Pixels, Rgba)>("wait-list")
            .mode(PresenceMode::Wait)
            .enter((px(0.0), rgba(0x24b47eff)))
            .exit((px(-80.0), rgba(0x24b47e00)))
            .transition(Tween::new(0.45).easing(easing::ease_in_out))
            .child(
                ElementId::named_usize("wait-item", wait_key),
                move |(offset, color)| {
                    div()
                        .relative()
                        .left(offset)
                        .px_3()
                        .py_1()
                        .rounded_md()
                        .bg(color)
                        .child(format!("Wait child {wait_key}"))
                        .into_any_element()
                },
            );

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_5()
            .bg(rgb(0x11131a))
            .text_color(rgb(0xf4f5f8))
            .child(div().text_2xl().child("gpui-motion v0.2 · element layer"))
            .child(
                section("MotionElement · velocity-preserving retarget")
                    .child(button(
                        "toggle-panel",
                        "Redirect spring",
                        cx.listener(|this, _, _, cx| {
                            this.panel_open = !this.panel_open;
                            cx.notify();
                        }),
                    ))
                    .child(
                        track().child(
                            div()
                                .absolute()
                                .top(px(8.0))
                                .w(px(86.0))
                                .h(px(34.0))
                                .rounded_md()
                                .with_motion(
                                    "spring-panel",
                                    panel_target,
                                    Spring::wobbly(),
                                    |panel, (left, color)| panel.left(left).bg(color),
                                )
                                .initial((px(16.0), rgb(0x24b47e))),
                        ),
                    ),
            )
            .child(
                section("Keyframes · custom times + per-segment easing")
                    .child(button(
                        "keyframes",
                        "Reverse path",
                        cx.listener(|this, _, _, cx| {
                            this.keyframes_forward = !this.keyframes_forward;
                            cx.notify();
                        }),
                    ))
                    .child(
                        track().child(
                            div()
                                .absolute()
                                .top(px(9.0))
                                .size(px(32.0))
                                .rounded_full()
                                .bg(rgb(0xffb020))
                                .with_motion(
                                    "keyframe-dot",
                                    keyframes,
                                    keyframes_timing,
                                    |dot, left| dot.left(left),
                                ),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        section("Repeat Mirror · breathing")
                            .child(
                                div()
                                    .size(px(58.0))
                                    .rounded_full()
                                    .bg(rgb(0xff5c8a))
                                    .with_motion(
                                        "breathing-light",
                                        1.0_f32,
                                        Transition::from(Tween::new(0.8))
                                            .repeat_forever(RepeatKind::Mirror),
                                        |light, opacity| light.opacity(opacity),
                                    )
                                    .initial(0.25),
                            )
                            .w_1_3(),
                    )
                    .child(
                        section("Per-property · wobbly position + color tween")
                            .child(button(
                                "per-property",
                                "Toggle",
                                cx.listener(|this, _, _, cx| {
                                    this.per_property_active = !this.per_property_active;
                                    cx.notify();
                                }),
                            ))
                            .child(
                                track().child(
                                    div()
                                        .absolute()
                                        .top(px(10.0))
                                        .size(px(30.0))
                                        .rounded_md()
                                        .with_motion(
                                            "per-property-dot",
                                            per_property_target,
                                            (Spring::wobbly(), Tween::new(0.25)),
                                            |dot, (left, color)| dot.left(left).bg(color),
                                        ),
                                ),
                            )
                            .flex_1(),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        section("whileHover / whilePress")
                            .child(
                                div()
                                    .w(px(160.0))
                                    .h(px(42.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_lg()
                                    .child("Point and press")
                                    .with_motion(
                                        "gesture-button",
                                        (px(160.0), rgba(0x303747ff)),
                                        (Spring::stiff(), Tween::new(0.15)),
                                        |button, (width, color)| button.w(width).bg(color),
                                    )
                                    .while_hover((px(190.0), rgba(0x7c5cffff)))
                                    .while_press((px(145.0), rgba(0xff5c8aff))),
                            )
                            .w_1_3(),
                    )
                    .child(
                        section("MotionValue · shared imperative value")
                            .child(format!("width: {:.0}px", f32::from(progress)))
                            .child(button(
                                "progress",
                                "Next target",
                                cx.listener(|this, _, _, cx| {
                                    const TARGETS: [f32; 4] = [64.0, 176.0, 320.0, 112.0];
                                    this.progress_step = (this.progress_step + 1) % TARGETS.len();
                                    this.progress.set_target(px(TARGETS[this.progress_step]));
                                    cx.notify();
                                }),
                            ))
                            .child(
                                div()
                                    .w(px(360.0))
                                    .h(px(14.0))
                                    .rounded_lg()
                                    .bg(rgb(0x151821))
                                    .child(
                                        div().w(progress).h_full().rounded_lg().bg(rgb(0x4cc9f0)),
                                    ),
                            )
                            .flex_1(),
                    ),
            )
            .child(
                section("DragTracker + MotionValue::jump + inertia flick").child(
                    div()
                        .id("drag-track")
                        .relative()
                        .w_full()
                        .h(px(62.0))
                        .rounded_md()
                        .bg(rgb(0x151821))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                this.drag.begin(event.position);
                                cx.notify();
                            }),
                        )
                        .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                            if this.drag.is_dragging() {
                                let delta = this.drag.update(event.position);
                                this.drag_position = px((f32::from(this.drag_position)
                                    + f32::from(delta.x))
                                .clamp(0.0, 460.0));
                                this.drag_x.jump(this.drag_position);
                                cx.notify();
                            }
                        }))
                        .on_mouse_up(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.finish_drag(cx)),
                        )
                        .on_mouse_up_out(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| this.finish_drag(cx)),
                        )
                        .child(
                            div()
                                .absolute()
                                .left(drag_x)
                                .top(px(8.0))
                                .w(px(120.0))
                                .h(px(46.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_lg()
                                .cursor_grab()
                                .bg(rgb(0xffb020))
                                .text_color(rgb(0x17120a))
                                .child("Drag / flick"),
                        ),
                ),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        section("presence · single exit retention")
                            .child(button(
                                "toast",
                                "Toggle toast",
                                cx.listener(|this, _, _, cx| {
                                    this.toast_visible = !this.toast_visible;
                                    cx.notify();
                                }),
                            ))
                            .child(presence(
                                "demo-toast",
                                self.toast_visible,
                                (px(0.0), rgba(0xffb020ff)),
                                (px(80.0), rgba(0xffb02000)),
                                Spring::stiff().into(),
                                |(left, color)| {
                                    div()
                                        .relative()
                                        .left(left)
                                        .px_3()
                                        .py_1()
                                        .rounded_md()
                                        .bg(color)
                                        .child("Saved successfully")
                                },
                            ))
                            .w_1_3(),
                    )
                    .child(
                        section("presence_group · Sync")
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .child(button(
                                        "add-item",
                                        "Add",
                                        cx.listener(|this, _, _, cx| {
                                            this.sync_items.push(this.next_item);
                                            this.next_item += 1;
                                            cx.notify();
                                        }),
                                    ))
                                    .child(button(
                                        "remove-item",
                                        "Remove",
                                        cx.listener(|this, _, _, cx| {
                                            this.sync_items.pop();
                                            cx.notify();
                                        }),
                                    )),
                            )
                            .child(sync_group)
                            .flex_1(),
                    )
                    .child(
                        section("presence_group · Wait")
                            .child(button(
                                "swap-wait",
                                "Swap",
                                cx.listener(|this, _, _, cx| {
                                    this.wait_first = !this.wait_first;
                                    cx.notify();
                                }),
                            ))
                            .child(wait_group)
                            .w_1_3(),
                    ),
            )
    }
}

fn section(title: &'static str) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .rounded_lg()
        .bg(rgb(0x1c202b))
        .child(title)
}

fn track() -> gpui::Div {
    div()
        .relative()
        .w_full()
        .h(px(50.0))
        .rounded_md()
        .bg(rgb(0x151821))
}

fn button(
    id: &'static str,
    label: &'static str,
    listener: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .w(px(132.0))
        .px_3()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .bg(rgb(0x303747))
        .hover(|style| style.bg(rgb(0x3d465a)))
        .on_click(listener)
        .child(label)
}

fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1060.0), px(980.0)), cx);
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
                    keyframes_forward: true,
                    per_property_active: false,
                    drag_x: MotionValue::new(px(40.0), Inertia::new().bounds(0.0, 460.0).into()),
                    drag_position: px(40.0),
                    drag: DragTracker::new(),
                    next_item: 4,
                    sync_items: vec![1, 2, 3],
                    wait_first: true,
                })
            },
        )
        .expect("failed to open demo window");
        cx.activate(true);
    });
}
