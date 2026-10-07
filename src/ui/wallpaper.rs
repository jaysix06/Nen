use crate::{models::Settings, theme};
use gpui_kit::{prelude::FluentBuilder, *};
use std::path::PathBuf;

/// Main and floating windows paint the same prepared image and appearance settings.
pub fn wallpaper(settings: &Settings, image: Option<PathBuf>, radius: f32) -> impl IntoElement {
    let color = u32::from_str_radix(settings.background_color.trim_start_matches('#'), 16)
        .unwrap_or(0xf6f5f1);
    div()
        .id("wallpaper-layer")
        .test_support()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .rounded(px(radius))
        .bg(rgb(color))
        .when_some(image, |view, path| {
            view.child(
                div()
                    .id("wallpaper-image")
                    .test_support()
                    .relative()
                    .size_full()
                    .child(
                        img(path)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .rounded(px(radius))
                            .object_fit(match settings.background_fit.as_str() {
                                "Contain" => ObjectFit::Contain,
                                "Center" => ObjectFit::None,
                                _ => ObjectFit::Cover,
                            })
                            .opacity(settings.background_opacity),
                    ),
            )
        })
        .child(
            div()
                .id("wallpaper-dim")
                .test_support()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .rounded(px(radius))
                .bg(rgb(0x000000).alpha(theme::background_dim(settings))),
        )
}
