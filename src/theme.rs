use crate::models::Settings;
use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;

#[derive(Clone, Copy)]
pub struct Palette {
    pub canvas: Hsla,
    pub sidebar: Hsla,
    pub paper: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub line: Hsla,
    pub selected: Hsla,
    pub accent: Hsla,
}

pub fn palette(cx: &App) -> Palette {
    if Theme::global(cx).is_dark() {
        Palette {
            canvas: rgb(0x1b1e1c).into(),
            sidebar: rgb(0x181b19).into(),
            paper: rgb(0x202420).into(),
            text: rgb(0xe6e8e0).into(),
            muted: rgb(0x989f94).into(),
            line: rgb(0x30362f).into(),
            selected: rgb(0x303b30).into(),
            accent: rgb(0xa6bea0).into(),
        }
    } else {
        Palette {
            canvas: rgb(0xf6f5f1).into(),
            sidebar: rgb(0xeeeee8).into(),
            paper: rgb(0xfcfbf8).into(),
            text: rgb(0x2b332c).into(),
            muted: rgb(0x606b59).into(),
            line: rgb(0xe2e5dc).into(),
            selected: rgb(0xe5ebdf).into(),
            accent: rgb(0x426446).into(),
        }
    }
}

pub fn apply(settings: &Settings, cx: &mut App) {
    match settings.theme.as_str() {
        "Dark" => Theme::change(ThemeMode::Dark, None, cx),
        "Light" => Theme::change(ThemeMode::Light, None, cx),
        _ => Theme::sync_system_appearance(None, cx),
    }
    let p = palette(cx);
    Theme::update(cx, |theme| {
        theme.font_family = "Segoe UI".into();
        theme.font_size = px(14.);
        theme.radius = px(5.);
        theme.radius_lg = px(10.);
        theme.colors.background = p.paper;
        theme.colors.foreground = p.text;
        theme.colors.primary = p.accent;
        theme.colors.primary_foreground = p.paper;
        theme.colors.border = p.line;
    });
    cx.set_reduce_motion(settings.reduced_motion || crate::platform::reduced_motion());
}

pub fn surfaces(settings: &Settings, cx: &App) -> Palette {
    let mut p = palette(cx);
    let (paper, sidebar, canvas) = match settings.surface.as_str() {
        "Frosted" => (0.94, 0.9, 0.8),
        "Clear" => (0.88, 0.85, 0.72),
        _ => (0.985, 0.96, 0.9),
    };
    p.paper = p.paper.alpha(paper);
    p.sidebar = p.sidebar.alpha(sidebar);
    p.canvas = p.canvas.alpha(canvas);
    if settings.surface == "Clear" {
        p.muted = if Theme::global(cx).is_dark() {
            rgb(0xb5bdad).into()
        } else {
            rgb(0x44533c).into()
        };
    }
    p
}

pub const SIDEBAR_WIDTH: f32 = 176.;
pub const LIST_WIDTH: f32 = 286.;
