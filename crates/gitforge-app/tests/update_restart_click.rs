use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{
    AppContext as _, Modifiers, MouseButton, Render, TestAppContext, VisualTestContext, Window,
    WindowControlArea, div, prelude::*, px,
};

struct TitlebarHarness;

impl Render for TitlebarHarness {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("titlebar")
            .w_full()
            .h(px(32.0))
            .window_control_area(WindowControlArea::Drag)
            .on_mouse_down(MouseButton::Left, |_event, window, _| {
                window.start_window_move();
            })
            .flex()
            .flex_row()
            .items_center()
            .justify_end()
            .child(
                div()
                    .id("update-indicator-restart")
                    .debug_selector(|| "update-indicator-restart".to_string())
                    .px_2()
                    .py_0p5()
                    .cursor_pointer()
                    .child("Restart to update")
                    // Mirrors update_indicator.rs: without this, titlebar drag eats the click.
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_click(|_, _, cx| gitforge_update::restart_to_apply_update(cx)),
            )
    }
}

#[gpui::test]
fn restart_button_click_works_inside_titlebar_drag_region(cx: &mut TestAppContext) {
    let restarted = Arc::new(AtomicBool::new(false));
    let flag = restarted.clone();
    let mut restart_subscription = None;

    cx.update(|app| {
        app.set_restart_path("/tmp/gitforge-restart-test".into());
        restart_subscription = Some(app.on_app_restart(move |_| {
            flag.store(true, Ordering::SeqCst);
        }));
    });

    let window = cx
        .update(|app| {
            app.open_window(Default::default(), |_window, cx| {
                cx.new(|_cx| TitlebarHarness)
            })
            .expect("open harness window")
        })
        .into();

    cx.run_until_parked();

    let mut window_cx = VisualTestContext::from_window(window, cx);
    let click_position = window_cx
        .debug_bounds("update-indicator-restart")
        .expect("restart button should be rendered")
        .center();

    window_cx.simulate_click(click_position, Modifiers::default());
    cx.run_until_parked();

    assert!(
        restarted.load(Ordering::SeqCst),
        "clicking the restart button inside the titlebar drag region should trigger restart"
    );
}
