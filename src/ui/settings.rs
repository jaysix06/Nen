use crate::{app::AppState, models::*, platform, theme::palette};
use gpui_kit::component::{
    button::*,
    input::{Input, InputEvent, InputState},
    menu::*,
    slider::{Slider, SliderEvent, SliderState},
    switch::Switch,
    *,
};
use gpui_kit::{assets::IconName, prelude::FluentBuilder, *};
use std::collections::HashMap;

const NAVIGATION: [(&str, &[(&str, IconName)]); 3] = [
    (
        "App",
        &[
            ("General", IconName::Settings),
            ("Reminders", IconName::Bell),
        ],
    ),
    (
        "Personalization",
        &[
            ("Appearance", IconName::Palette),
            ("Background", IconName::Image),
            ("Floating bar", IconName::AppWindow),
        ],
    ),
    (
        "Controls",
        &[
            ("Shortcuts", IconName::Keyboard),
            ("Advanced", IconName::SlidersHorizontal),
        ],
    ),
];
pub struct SettingsView {
    state: Entity<AppState>,
    sliders: HashMap<&'static str, Entity<SliderState>>,
    color: Entity<InputState>,
    reminder_time: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}
impl SettingsView {
    pub fn new(state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let settings = state.read(cx).settings.clone();
        let color = cx
            .new(|cx| InputState::new(window, cx).default_value(settings.background_color.clone()));
        let reminder_time = cx.new(|cx| {
            InputState::new(window, cx).default_value(settings.default_reminder_time.clone())
        });
        let mut view = Self {
            state: state.clone(),
            sliders: HashMap::new(),
            color: color.clone(),
            reminder_time: reminder_time.clone(),
            _subscriptions: Vec::new(),
        };
        view._subscriptions
            .push(cx.observe_in(&state, window, |view, _, window, cx| {
                let settings = view.state.read(cx).settings.clone();
                if !view
                    .reminder_time
                    .read(cx)
                    .focus_handle(cx)
                    .is_focused(window)
                    && view.reminder_time.read(cx).value().as_ref()
                        != settings.default_reminder_time
                {
                    view.reminder_time.update(cx, |input, cx| {
                        input.set_value(settings.default_reminder_time.clone(), window, cx)
                    });
                }
                for (key, slider) in &view.sliders {
                    let value = match *key {
                        "font" => settings.editor_font_size,
                        "width" => settings.floating_width,
                        "opacity" => settings.floating_opacity,
                        "blur" => settings.background_blur,
                        "dim" => settings.background_dim,
                        "saturation" => settings.background_saturation,
                        _ => settings.background_opacity,
                    };
                    if (slider.read(cx).value().start() - value).abs() > f32::EPSILON {
                        slider.update(cx, |slider, cx| slider.set_value(value, window, cx));
                    }
                }
                if !view.color.read(cx).focus_handle(cx).is_focused(window)
                    && view.color.read(cx).value().as_ref() != settings.background_color
                {
                    view.color.update(cx, |input, cx| {
                        input.set_value(settings.background_color, window, cx)
                    });
                }
                cx.notify();
            }));
        for (key, min, max, step, value) in [
            ("font", 12., 28., 1., settings.editor_font_size),
            ("width", 420., 760., 10., settings.floating_width),
            ("opacity", 0.6, 1., 0.05, settings.floating_opacity),
            ("blur", 0., 40., 1., settings.background_blur),
            ("dim", 0., 0.8, 0.05, settings.background_dim),
            ("saturation", 0., 2., 0.1, settings.background_saturation),
            ("image_opacity", 0., 1., 0.05, settings.background_opacity),
        ] {
            let slider = cx.new(|_| {
                SliderState::new()
                    .max(max)
                    .min(min)
                    .step(step)
                    .default_value(value)
            });
            view._subscriptions
                .push(cx.subscribe(&slider, move |view, _, event, cx| {
                    if let SliderEvent::Change(value) = event {
                        let value = value.start();
                        view.state.update(cx, |state, cx| {
                            let mut settings = state.settings.clone();
                            match key {
                                "font" => settings.editor_font_size = value,
                                "width" => settings.floating_width = value,
                                "opacity" => settings.floating_opacity = value,
                                "blur" => settings.background_blur = value,
                                "dim" => settings.background_dim = value,
                                "saturation" => settings.background_saturation = value,
                                _ => settings.background_opacity = value,
                            }
                            state.update_settings(settings, cx);
                        });
                    }
                }));
            view.sliders.insert(key, slider);
        }
        view._subscriptions
            .push(cx.subscribe(&color, |view, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    let color = view.color.read(cx).value().to_string();
                    if color.len() == 7
                        && color.starts_with('#')
                        && u32::from_str_radix(&color[1..], 16).is_ok()
                    {
                        view.state.update(cx, |state, cx| {
                            let mut settings = state.settings.clone();
                            settings.background_color = color;
                            state.update_settings(settings, cx);
                        });
                    }
                }
            }));
        view._subscriptions
            .push(cx.subscribe(&reminder_time, |view, _, event, cx| {
                if matches!(event, InputEvent::Change) {
                    let time = view.reminder_time.read(cx).value().to_string();
                    if time.len() == 5 && chrono::NaiveTime::parse_from_str(&time, "%H:%M").is_ok()
                    {
                        view.state.update(cx, |state, cx| {
                            let mut settings = state.settings.clone();
                            settings.default_reminder_time = time;
                            state.update_settings(settings, cx);
                        });
                    }
                }
            }));
        let weak = cx.weak_entity();
        let handle = window.window_handle();
        view._subscriptions
            .push(cx.intercept_keystrokes(move |event, window, cx| {
                if window.window_handle() == handle && !window.has_active_dialog(cx) {
                    let _ = weak.update(cx, |view, cx| view.capture(&event.keystroke, cx));
                }
            }));
        view
    }
    fn capture(&mut self, key: &Keystroke, cx: &mut Context<Self>) {
        let Some(action) = self.state.read(cx).shortcut_capture.clone() else {
            return;
        };
        cx.stop_propagation();
        if key.key == "escape" {
            self.state.update(cx, |state, cx| {
                state.shortcut_capture = None;
                cx.notify();
            });
            return;
        }
        if matches!(
            key.key.as_str(),
            "control" | "alt" | "shift" | "super" | "command" | "win"
        ) {
            return;
        }
        let function_key = key
            .key
            .strip_prefix('f')
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=24).contains(&number));
        if !key.modifiers.modified() && !function_key {
            return;
        }
        let binding = key.to_string().to_ascii_lowercase();
        self.state.update(cx, |state, cx| {
            match set_binding(state, &action, &binding) {
                Ok(()) => {
                    state.shortcut_capture = None;
                    let settings = state.settings.clone();
                    state.update_settings(settings, cx);
                    state.toast("Shortcut updated", cx);
                }
                Err(error) => state.fail(error, cx),
            }
        });
    }
    fn toggle(&self, key: &'static str, label: &'static str, value: bool, cx: &App) -> AnyElement {
        let state = self.state.clone();
        row(
            label,
            Switch::new(key)
                .checked(value)
                .on_change(move |value, _, cx| {
                    state.update(cx, |state, cx| {
                        let mut settings = state.settings.clone();
                        match key {
                            "startup" => {
                                if let Err(error) = platform::set_startup(*value) {
                                    state.fail(format!("Couldn't change startup: {error}"), cx);
                                    return;
                                }
                                settings.launch_at_startup = *value;
                            }
                            "tray" => settings.minimize_to_tray = *value,
                            "restore" => settings.restore_tabs = *value,
                            "start_tray" => settings.start_in_tray = *value,
                            "float_enabled" => settings.floating_enabled = *value,
                            "float_start" => settings.floating_on_startup = *value,
                            "topmost" => settings.floating_topmost = *value,
                            "hide_blur" => settings.floating_hide_on_blur = *value,
                            "remember_position" => settings.floating_remember_position = *value,
                            "sound" => settings.notification_sound = *value,
                            "motion" => settings.reduced_motion = *value,
                            _ => {}
                        }
                        state.update_settings(settings, cx);
                        if key == "float_enabled" && !*value {
                            let entity = cx.entity();
                            cx.defer(move |cx| super::floating::show(entity, false, cx));
                        }
                    })
                }),
            cx,
        )
    }
    fn choice(
        &self,
        key: &'static str,
        label: &'static str,
        value: String,
        choices: Vec<&'static str>,
        cx: &App,
    ) -> AnyElement {
        let state = self.state.clone();
        row(
            label,
            Button::new(key)
                .label(value)
                .icon(IconName::ChevronDown)
                .w(px(165.))
                .dropdown_menu(move |mut menu, _, _| {
                    for choice in &choices {
                        let choice = *choice;
                        let state = state.clone();
                        menu = menu.item(PopupMenuItem::new(choice).on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                let mut settings = state.settings.clone();
                                match key {
                                    "theme" => settings.theme = choice.into(),
                                    "accent" => settings.accent_color = choice.into(),
                                    "surface" => settings.surface = choice.into(),
                                    "position" => settings.floating_position = choice.into(),
                                    "fit" => settings.background_fit = choice.into(),
                                    "type" => {
                                        settings.default_note_type = if choice == "Scratch" {
                                            NoteType::Scratch
                                        } else {
                                            NoteType::Normal
                                        }
                                    }
                                    "snooze" => {
                                        settings.snooze_minutes = choice.parse().unwrap_or(10)
                                    }
                                    _ => {}
                                }
                                state.update_settings(settings, cx);
                            })
                        }));
                    }
                    menu
                }),
            cx,
        )
    }
    fn range(&self, key: &'static str, label: &'static str, cx: &App) -> AnyElement {
        let slider = &self.sliders[key];
        let value = slider.read(cx).value().start();
        row(
            label,
            div()
                .id(format!("settings-slider-{key}"))
                .test_support()
                .flex()
                .items_center()
                .gap_4()
                .w(px(if key == "font" { 292. } else { 220. }))
                .child(Slider::new(slider).flex_1())
                .child(
                    div()
                        .w(px(36.))
                        .text_xs()
                        .text_color(crate::theme::surfaces(&self.state.read(cx).settings, cx).muted)
                        .child(if key == "font" || key == "width" || key == "blur" {
                            format!("{value:.0}")
                        } else {
                            format!("{:.0}%", value * 100.)
                        }),
                )
                .when(key == "font", |view| {
                    view.child(
                        div()
                            .id("editor-font-preview")
                            .test_support()
                            .w(px(56.))
                            .flex_shrink_0()
                            .text_size(px(value))
                            .line_height(relative(1.55))
                            .text_color(
                                crate::theme::surfaces(&self.state.read(cx).settings, cx).text,
                            )
                            .child("Aa"),
                    )
                }),
            cx,
        )
    }
}

pub fn set_binding(state: &mut AppState, action: &str, binding: &str) -> Result<(), String> {
    if let Some((conflict, _)) = state
        .settings
        .shortcuts
        .iter()
        .find(|(key, value)| key.as_str() != action && value.as_str() == binding)
    {
        return Err(format!(
            "This shortcut is assigned to {}.",
            action_label(conflict)
        ));
    }
    if ["toggle_float", "quick_note", "open_app"].contains(&action)
        && let Some(desktop) = state.desktop.as_mut()
    {
        desktop
            .change_hotkey(action, binding)
            .map_err(|e| e.to_string())?;
    }
    state
        .settings
        .shortcuts
        .insert(action.into(), binding.into());
    Ok(())
}
pub fn action_label(action: &str) -> String {
    if let Some(index) = action.strip_prefix("tab_") {
        return format!("Switch to tab {index}");
    }
    match action {
        "new_note" => "New note",
        "close_tab" => "Close tab",
        "reopen_tab" => "Reopen closed tab",
        "next_tab" => "Next tab",
        "previous_tab" => "Previous tab",
        "toggle_sidebar" => "Toggle notes sidebar",
        "find" => "Find in note",
        "search" => "Search all notes",
        "reminder" => "Set reminder",
        "toggle_float" => "Toggle floating bar",
        "open_app" => "Open full app",
        "quick_note" => "Quick note",
        "settings" => "Settings",
        "pin" => "Pin / unpin note",
        "archive" => "Archive / restore note",
        _ => action,
    }
    .into()
}
fn row(label: impl Into<SharedString>, control: impl IntoElement, cx: &App) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_5()
        .min_h(px(48.))
        .border_b_1()
        .border_color(palette(cx).line)
        .child(div().text_sm().child(label.into()))
        .child(control)
        .into_any_element()
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = crate::theme::surfaces(&self.state.read(cx).settings, cx);
        let settings = self.state.read(cx).settings.clone();
        let section = self
            .state
            .read(cx)
            .settings_page
            .clone()
            .unwrap_or("General".into());
        let mut nav = div()
            .w(px(208.))
            .flex_shrink_0()
            .v_flex()
            .gap_1()
            .px_3()
            .py_4()
            .border_r_1()
            .border_color(p.line)
            .bg(p.canvas)
            .child(
                div()
                    .px_2()
                    .text_size(px(13.))
                    .font_semibold()
                    .child("Settings"),
            );
        for (group, sections) in NAVIGATION {
            nav = nav.child(
                div()
                    .px_2()
                    .mt_4()
                    .mb_1()
                    .text_size(px(10.))
                    .font_semibold()
                    .text_color(p.muted)
                    .child(group),
            );
            for (name, icon) in sections {
                let name = *name;
                let state = self.state.clone();
                nav = nav.child(
                    Button::new(name)
                        .ghost()
                        .accessibility_label(name)
                        .w_full()
                        .h(px(32.))
                        .px_2()
                        .child(
                            div()
                                .id(format!("settings-nav-{name}"))
                                .test_support()
                                .w_full()
                                .flex()
                                .items_center()
                                .justify_start()
                                .gap_2()
                                .child(Icon::new(*icon).size_4())
                                .child(
                                    div()
                                        .id(format!("settings-nav-label-{name}"))
                                        .test_support()
                                        .text_size(px(13.))
                                        .child(name),
                                ),
                        )
                        .when(name == section, |button| {
                            button.bg(p.selected).text_color(p.accent)
                        })
                        .on_click(move |_, _, cx| {
                            state.update(cx, |state, cx| {
                                state.settings_page = Some(name.into());
                                state.shortcut_capture = None;
                                cx.notify();
                            })
                        }),
                );
            }
        }
        let close = self.state.clone();
        nav = nav.child(div().flex_1()).child(
            Button::new("close-settings")
                .ghost()
                .w_full()
                .accessibility_label("Back to notes")
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(Icon::new(IconName::ArrowLeft).size_4())
                        .child(div().text_size(px(13.)).child("Back to notes")),
                )
                .on_click(move |_, _, cx| {
                    close.update(cx, |state, cx| {
                        state.settings_page = None;
                        state.shortcut_capture = None;
                        cx.notify();
                    })
                }),
        );
        let mut content = div()
            .id("settings-content")
            .flex_1()
            .min_w_0()
            .v_flex()
            .overflow_y_scroll()
            .px_8()
            .py_5()
            .bg(p.paper)
            .child(
                div()
                    .text_size(px(18.))
                    .font_semibold()
                    .mb_5()
                    .child(section.clone()),
            );
        match section.as_str() {
            "General" => {
                content=content
                .child(self.toggle("startup","Launch at startup",settings.launch_at_startup,cx))
                .child(self.toggle("start_tray","Start in tray",settings.start_in_tray,cx))
                .child(self.toggle("tray","Close to tray",settings.minimize_to_tray,cx))
                .child(self.toggle("restore","Restore previous tabs",settings.restore_tabs,cx))
                .child(self.choice("type","New note type",if settings.default_note_type==NoteType::Normal {"Normal"}else{"Scratch"}.into(),vec!["Normal","Scratch"],cx))
                .child(div().mt_4().text_xs().text_color(p.muted).child("Reminders continue while Still is in the tray. Quitting stops them until Still opens again."));
            }
            "Appearance" => {
                content = content
                    .child(self.choice(
                        "theme",
                        "Theme",
                        settings.theme,
                        vec!["Light", "Dark", "System"],
                        cx,
                    ))
                    .child(self.choice(
                        "surface",
                        "Surface",
                        settings.surface,
                        vec!["Opaque", "Frosted", "Clear"],
                        cx,
                    ))
                    .child(self.choice(
                        "accent",
                        "Accent",
                        settings.accent_color,
                        vec!["Green", "Blue", "Amber", "Rose"],
                        cx,
                    ))
                    .child(self.range("font", "Editor font size", cx))
                    .child(self.toggle("motion", "Reduce motion", settings.reduced_motion, cx));
            }
            "Background" => {
                let choose = self.state.clone();
                let remove = self.state.clone();
                let restore = self.state.clone();
                content = content
                    .child(row(
                        "Solid color",
                        Input::new(&self.color)
                            .w(px(165.))
                            .aria_label("Background color"),
                        cx,
                    ))
                    .child(row(
                        "Local image",
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                Button::new("default-wallpaper")
                                    .ghost()
                                    .small()
                                    .label("Still")
                                    .tooltip("Use the bundled wallpaper")
                                    .on_click(move |_, _, cx| {
                                        restore.update(cx, |state, cx| {
                                            let mut settings = state.settings.clone();
                                            settings.background_image = None;
                                            settings.default_wallpaper = true;
                                            state.update_settings(settings, cx);
                                        })
                                    }),
                            )
                            .child(Button::new("choose-image").label("Choose image").on_click(
                                move |_, _, cx| {
                                    let paths = cx.prompt_for_paths(PathPromptOptions {
                                        files: true,
                                        directories: false,
                                        multiple: false,
                                        prompt: Some("Choose a background image".into()),
                                    });
                                    let state = choose.clone();
                                    cx.spawn(async move |cx| match paths.await {
                                        Ok(Ok(Some(paths))) => {
                                            if let Some(path) = paths.first() {
                                                let path = path.clone();
                                                let result = cx
                                                    .background_executor()
                                                    .spawn(async move {
                                                        crate::diagnostics::data_directory()
                                                            .and_then(|dir| {
                                                                crate::background::import(
                                                                    &path, &dir,
                                                                )
                                                            })
                                                    })
                                                    .await;
                                                state.update(cx, |state, cx| match result {
                                                    Ok(path) => {
                                                        let mut settings = state.settings.clone();
                                                        settings.background_image =
                                                            Some(path.to_string_lossy().into());
                                                        state.update_settings(settings, cx);
                                                    }
                                                    Err(error) => state.fail(
                                                        format!(
                                                            "Couldn't open this image: {error}"
                                                        ),
                                                        cx,
                                                    ),
                                                });
                                            }
                                        }
                                        Ok(Err(error)) => {
                                            state.update(cx, |state, cx| {
                                                state.fail(
                                                    format!(
                                                        "Couldn't open the file picker: {error}"
                                                    ),
                                                    cx,
                                                )
                                            });
                                        }
                                        _ => {}
                                    })
                                    .detach();
                                },
                            ))
                            .child(
                                Button::new("remove-image")
                                    .ghost()
                                    .icon(IconName::X)
                                    .tooltip("Remove image")
                                    .disabled(
                                        settings.background_image.is_none()
                                            && !settings.default_wallpaper,
                                    )
                                    .on_click(move |_, _, cx| {
                                        remove.update(cx, |state, cx| {
                                            let mut settings = state.settings.clone();
                                            settings.background_image = None;
                                            settings.default_wallpaper = false;
                                            state.update_settings(settings, cx);
                                        })
                                    }),
                            ),
                        cx,
                    ))
                    .child(self.choice(
                        "fit",
                        "Fit",
                        settings.background_fit,
                        vec!["Cover", "Contain", "Center"],
                        cx,
                    ))
                    .child(self.range("blur", "Blur", cx))
                    .child(self.range("dim", "Dim", cx))
                    .child(self.range("saturation", "Saturation", cx))
                    .child(self.range("image_opacity", "Opacity", cx));
            }
            "Floating bar" => {
                let show = self.state.clone();
                content = content
                    .child(self.toggle(
                        "float_enabled",
                        "Enable floating bar",
                        settings.floating_enabled,
                        cx,
                    ))
                    .child(self.toggle("topmost", "Always on top", settings.floating_topmost, cx))
                    .child(self.toggle(
                        "remember_position",
                        "Remember position",
                        settings.floating_remember_position,
                        cx,
                    ))
                    .child(self.toggle(
                        "hide_blur",
                        "Hide when focus is lost",
                        settings.floating_hide_on_blur,
                        cx,
                    ))
                    .child(self.toggle(
                        "float_start",
                        "Show on startup",
                        settings.floating_on_startup,
                        cx,
                    ))
                    .child(self.choice(
                        "position",
                        "Position",
                        settings.floating_position,
                        vec!["Top left", "Top center", "Top right", "Center", "Custom"],
                        cx,
                    ))
                    .child(self.range("width", "Width", cx))
                    .child(self.range("opacity", "Opacity", cx))
                    .child(
                        Button::new("preview-floating")
                            .mt_5()
                            .label("Show floating bar")
                            .on_click(move |_, _, cx| {
                                super::floating::show(show.clone(), true, cx)
                            }),
                    );
            }
            "Reminders" => {
                content = content
                    .child(row(
                        "Default time",
                        Input::new(&self.reminder_time)
                            .w(px(165.))
                            .aria_label("Default reminder time, HH:MM"),
                        cx,
                    ))
                    .child(self.choice(
                        "snooze",
                        "Snooze minutes",
                        settings.snooze_minutes.to_string(),
                        vec!["5", "10", "15", "30", "60"],
                        cx,
                    ))
                    .child(self.toggle(
                        "sound",
                        "Notification sound",
                        settings.notification_sound,
                        cx,
                    ));
            }
            "Shortcuts" => {
                for (action, binding) in &settings.shortcuts {
                    let state = self.state.clone();
                    let action = action.clone();
                    let reset = self.state.clone();
                    let reset_action = action.clone();
                    let capturing = self.state.read(cx).shortcut_capture.as_ref() == Some(&action);
                    let label = action_label(&action);
                    content = content.child(row(
                        label,
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Button::new(SharedString::from(format!("shortcut-{action}")))
                                    .label(if capturing {
                                        "Press shortcut… Esc cancels".into()
                                    } else {
                                        binding.replace('-', " + ")
                                    })
                                    .w(px(240.))
                                    .when(capturing, |button| {
                                        button.bg(p.selected).text_color(p.accent)
                                    })
                                    .on_click(move |_, _, cx| {
                                        state.update(cx, |state, cx| {
                                            state.shortcut_capture = Some(action.clone());
                                            state.error = None;
                                            cx.notify();
                                        })
                                    }),
                            )
                            .child(
                                Button::new(SharedString::from(format!("reset-{reset_action}")))
                                    .ghost()
                                    .small()
                                    .icon(IconName::RotateCcw)
                                    .tooltip("Reset to default")
                                    .on_click(move |_, _, cx| {
                                        reset.update(cx, |state, cx| {
                                            if let Some(default) =
                                                default_shortcuts().get(&reset_action)
                                            {
                                                match set_binding(state, &reset_action, default) {
                                                    Ok(()) => {
                                                        let settings = state.settings.clone();
                                                        state.update_settings(settings, cx);
                                                        state.toast("Shortcut reset", cx);
                                                    }
                                                    Err(error) => state.fail(error, cx),
                                                }
                                            }
                                        })
                                    }),
                            ),
                        cx,
                    ));
                }
                let state = self.state.clone();
                content = content.child(
                    Button::new("reset-shortcuts")
                        .mt_5()
                        .label("Reset all shortcuts")
                        .on_click(move |_, window, cx| {
                            let state = state.clone();
                            window.open_alert_dialog(cx, move |dialog, _, _| {
                                let state = state.clone();
                                dialog
                                    .confirm()
                                    .title("Reset all shortcuts?")
                                    .button_props(
                                        dialog::DialogButtonProps::default()
                                            .ok_text("Reset")
                                            .show_cancel(true),
                                    )
                                    .on_ok(move |_, _, cx| {
                                        state.update(cx, |state, cx| {
                                            let defaults = default_shortcuts();
                                            if let Some(desktop) = state.desktop.as_mut()
                                                && let Err(error) =
                                                    desktop.replace_hotkeys(&defaults)
                                            {
                                                state.fail(error.to_string(), cx);
                                                return;
                                            }
                                            let mut settings = state.settings.clone();
                                            settings.shortcuts = defaults;
                                            state.update_settings(settings, cx);
                                            state.toast("Shortcuts reset", cx);
                                        });
                                        true
                                    })
                            });
                        }),
                );
            }
            _ => {
                let state = self.state.clone();
                content = content
                    .child(
                        div()
                            .text_sm()
                            .text_color(p.muted)
                            .child("Notes and backgrounds stay on this computer."),
                    )
                    .child(
                        Button::new("reset-settings")
                            .mt_5()
                            .label("Reset preferences")
                            .on_click(move |_, window, cx| {
                                let state = state.clone();
                                window.open_alert_dialog(cx, move |dialog, _, _| {
                                    let state = state.clone();
                                    dialog
                                        .confirm()
                                        .title("Reset preferences?")
                                        .child("Your notes, reminders and shortcuts will be kept.")
                                        .button_props(
                                            dialog::DialogButtonProps::default()
                                                .ok_text("Reset")
                                                .show_cancel(true),
                                        )
                                        .on_ok(move |_, _, cx| {
                                            state.update(cx, |state, cx| {
                                                // Shortcut registration has its own atomic reset flow.
                                                let shortcuts = state.settings.shortcuts.clone();
                                                if let Err(error) = platform::set_startup(false) {
                                                    state.fail(error.to_string(), cx);
                                                    return;
                                                }
                                                state.update_settings(
                                                    Settings {
                                                        shortcuts,
                                                        ..Default::default()
                                                    },
                                                    cx,
                                                );
                                                state.toast("Settings reset", cx);
                                            });
                                            true
                                        })
                                });
                            }),
                    );
            }
        }
        div()
            .size_full()
            .flex()
            .text_color(p.text)
            .child(nav)
            .child(content)
    }
}
