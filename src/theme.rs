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
            canvas: rgb(0x111715).into(),
            sidebar: rgb(0x111613).into(),
            paper: rgb(0x121815).into(),
            text: rgb(0xe2e8df).into(),
            muted: rgb(0xb2bdb3).into(),
            line: rgb(0xffffff).alpha(0.08).into(),
            selected: rgb(0x9db794).alpha(0.12).into(),
            accent: Theme::global(cx).colors.primary,
        }
    } else {
        Palette {
            canvas: rgb(0xcbd5cb).into(),
            sidebar: rgb(0xd0dacc).into(),
            paper: rgb(0xdfe5da).into(),
            text: rgb(0x1d2d23).into(),
            muted: rgb(0x4b594e).into(),
            line: rgb(0x000000).alpha(0.08).into(),
            selected: rgb(0x426446).alpha(0.1).into(),
            accent: Theme::global(cx).colors.primary,
        }
    }
}

pub fn apply(settings: &Settings, cx: &mut App) {
    match settings.theme.as_str() {
        "Dark" => Theme::change(ThemeMode::Dark, None, cx),
        "Light" => Theme::change(ThemeMode::Light, None, cx),
        _ => Theme::sync_system_appearance(None, cx),
    }
    let mut p = palette(cx);
    p.accent = rgb(
        match (settings.accent_color.as_str(), Theme::global(cx).is_dark()) {
            ("Blue", true) => 0xa1bed0,
            ("Blue", false) => 0x3e617b,
            ("Amber", true) => 0xc7b68d,
            ("Amber", false) => 0x735b2d,
            ("Rose", true) => 0xc5a4ad,
            ("Rose", false) => 0x7d4d5b,
            (_, true) => 0xa5c19d,
            (_, false) => 0x426446,
        },
    )
    .into();
    Theme::update(cx, |theme| {
        theme.font_family = "Segoe UI".into();
        theme.font_size = px(13.);
        theme.radius = px(5.);
        theme.radius_lg = px(10.);
        theme.colors.background = p.paper;
        theme.colors.foreground = p.text;
        theme.colors.primary = p.accent;
        theme.colors.primary_foreground = p.paper;
        theme.colors.border = p.line;
        theme.colors.title_bar = p.canvas;
        theme.colors.title_bar_border = p.line;
        theme.colors.popover = p.sidebar;
        theme.colors.popover_foreground = p.text;
    });
    cx.set_reduce_motion(settings.reduced_motion || crate::platform::reduced_motion());
}

pub fn surfaces(settings: &Settings, cx: &App) -> Palette {
    let mut p = palette(cx);
    let (mut paper, mut sidebar, mut canvas) =
        match (settings.surface.as_str(), Theme::global(cx).is_dark()) {
            ("Frosted", true) => (0.42, 0.66, 0.22),
            ("Clear", true) => (0.28, 0.48, 0.16),
            ("Frosted", false) => (0.72, 0.84, 0.62),
            ("Clear", false) => (0.68, 0.8, 0.58),
            _ => (0.98, 0.97, 0.96),
        };
    // Protect foreground contrast on arbitrary (including white) backgrounds
    // through the surfaces, without imposing a dead zone on the Dim control.
    if Theme::global(cx).is_dark()
        && (settings.background_image.is_some() || !settings.default_wallpaper)
    {
        let minimum = if settings.surface == "Frosted" {
            0.82
        } else {
            0.78
        };
        paper = f32::max(paper, minimum);
        sidebar = f32::max(sidebar, minimum);
        canvas = f32::max(canvas, minimum);
    } else if settings.background_image.is_some() || !settings.default_wallpaper {
        canvas = f32::max(canvas, 0.78);
    }
    p.paper = p.paper.alpha(paper);
    p.sidebar = p.sidebar.alpha(sidebar);
    p.canvas = p.canvas.alpha(canvas);
    if !Theme::global(cx).is_dark() && settings.surface == "Frosted" {
        p.muted = rgb(0x24382c).into();
    } else if settings.surface == "Clear" {
        p.muted = if Theme::global(cx).is_dark() {
            rgb(0xb5bdad).into()
        } else {
            rgb(0x1c3022).into()
        };
    }
    p
}

pub fn background_dim(settings: &Settings) -> f32 {
    settings.background_dim.clamp(0., 1.)
}

pub const LIST_WIDTH: f32 = 272.;

pub fn island_radius(height: f32) -> f32 {
    (28. - (height - 56.) / 5.).clamp(18., 28.)
}
