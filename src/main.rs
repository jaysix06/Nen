use gpui_kit::component::*;
use gpui_kit::*;

struct Notes;
impl Render for Notes {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0xf6f5f1))
            .text_color(rgb(0x292e2a))
            .v_flex()
            .items_center()
            .justify_center()
            .child("Still")
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1180.), px(780.)),
                    cx,
                ))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Still".into()),
                    ..Default::default()
                }),
                window_min_size: Some(size(px(820.), px(540.))),
                app_id: Some("dev.still.notes".into()),
                ..Default::default()
            };
            if let Err(error) = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Notes)) {
                eprintln!("Could not open Still: {error}");
                cx.quit();
            }
        });
}
