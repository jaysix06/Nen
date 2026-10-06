use crate::{app::AppState, models::*, storage::Request, theme::palette};
use chrono::{Duration as DateDuration, Local, NaiveDate, NaiveTime, TimeZone};
use gpui_kit::component::{
    button::*,
    input::{Input, InputState},
    menu::*,
    *,
};
use gpui_kit::{assets::IconName, *};

pub struct ReminderPicker {
    state: Entity<AppState>,
    note_id: String,
    date: Entity<InputState>,
    time: Entity<InputState>,
    days: Entity<InputState>,
    recurrence: Recurrence,
    advanced: bool,
    error: Option<String>,
    inline: bool,
}

impl ReminderPicker {
    pub fn focus_date(&self, window: &mut Window, cx: &mut App) {
        self.date.update(cx, |input, cx| input.focus(window, cx));
    }
    pub fn preferred_height(&self) -> f32 {
        let height = if !self.advanced {
            304.
        } else if matches!(self.recurrence, Recurrence::EveryDays(_)) {
            416.
        } else {
            368.
        };
        height + if self.error.is_some() { 48. } else { 0. }
    }
    pub fn new(
        state: Entity<AppState>,
        note_id: String,
        inline: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let tomorrow = (Local::now() + DateDuration::days(1))
            .format("%Y-%m-%d")
            .to_string();
        let default_time = state.read(cx).settings.default_reminder_time.clone();
        Self {
            state,
            note_id,
            date: cx.new(|cx| InputState::new(window, cx).default_value(tomorrow)),
            time: cx.new(|cx| InputState::new(window, cx).default_value(default_time)),
            days: cx.new(|cx| InputState::new(window, cx).default_value("3")),
            recurrence: Recurrence::Never,
            advanced: false,
            error: None,
            inline,
        }
    }
    fn preset(&mut self, minutes: i64, window: &mut Window, cx: &mut Context<Self>) {
        let now = Local::now();
        let target = match minutes {
            -1 => now
                .date_naive()
                .and_hms_opt(20, 0, 0)
                .and_then(|n| Local.from_local_datetime(&n).earliest())
                .map(|d| {
                    if d <= now {
                        d + DateDuration::days(1)
                    } else {
                        d
                    }
                }),
            -2 | -3 | -7 => (now + DateDuration::days(if minutes == -7 { 7 } else { 1 }))
                .date_naive()
                .and_hms_opt(if minutes == -3 { 14 } else { 9 }, 0, 0)
                .and_then(|n| Local.from_local_datetime(&n).earliest()),
            _ => Some(now + DateDuration::minutes(minutes)),
        };
        if let Some(target) = target {
            self.date.update(cx, |input, cx| {
                input.set_value(target.format("%Y-%m-%d").to_string(), window, cx)
            });
            self.time.update(cx, |input, cx| {
                input.set_value(target.format("%H:%M").to_string(), window, cx)
            });
            self.error = None;
            cx.notify();
        }
    }
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let date = NaiveDate::parse_from_str(self.date.read(cx).value().as_ref(), "%Y-%m-%d");
        let time = NaiveTime::parse_from_str(self.time.read(cx).value().as_ref(), "%H:%M");
        let target = date
            .ok()
            .zip(time.ok())
            .and_then(|(d, t)| Local.from_local_datetime(&d.and_time(t)).earliest());
        let Some(target) = target.filter(|d| d.timestamp() > Local::now().timestamp()) else {
            self.error = Some("Choose a future date and time. Use YYYY-MM-DD and HH:MM.".into());
            cx.notify();
            return;
        };
        let recurrence = if matches!(self.recurrence, Recurrence::EveryDays(_)) {
            match self.days.read(cx).value().parse::<u32>() {
                Ok(n) if (1..=3650).contains(&n) => Recurrence::EveryDays(n),
                _ => {
                    self.error = Some("Enter between 1 and 3650 days.".into());
                    cx.notify();
                    return;
                }
            }
        } else {
            self.recurrence.clone()
        };
        let id = self.note_id.clone();
        self.state.update(cx, |state, cx| {
            state.save_now(&id, cx);
            state.perform(
                Request::AddReminder(Reminder {
                    id: uuid::Uuid::new_v4().to_string(),
                    note_id: id,
                    scheduled_at: target.timestamp(),
                    recurrence,
                    status: "pending".into(),
                    title: String::new(),
                    preview: String::new(),
                    series_id: None,
                }),
                "Reminder created",
                cx,
            );
        });
        if self.inline {
            self.state.update(cx, |state, cx| {
                state.floating_reminder = false;
                cx.notify();
            });
        } else {
            window.close_dialog(cx);
        }
    }
}

impl Render for ReminderPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = palette(cx);
        let mut presets = div().flex().flex_wrap().gap_2();
        for (label, minutes) in [
            ("In 30 minutes", 30),
            ("In 1 hour", 60),
            ("Tonight", -1),
            ("Tomorrow morning", -2),
            ("Tomorrow afternoon", -3),
            ("Next week", -7),
        ] {
            presets = presets.child(Button::new(label).small().label(label).on_click(
                cx.listener(move |view, _, window, cx| view.preset(minutes, window, cx)),
            ));
        }
        let mut content = div()
            .v_flex()
            .gap_4()
            .text_color(p.text)
            .child(presets)
            .child(
                div()
                    .flex()
                    .gap_3()
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .flex_1()
                            .child(div().text_xs().text_color(p.muted).child("Date"))
                            .child(Input::new(&self.date).aria_label("Reminder date")),
                    )
                    .child(
                        div()
                            .v_flex()
                            .gap_2()
                            .w(px(110.))
                            .child(div().text_xs().text_color(p.muted).child("Time"))
                            .child(Input::new(&self.time).aria_label("Reminder time")),
                    ),
            )
            .child(
                Button::new("repeat-options")
                    .ghost()
                    .small()
                    .label(if self.advanced {
                        "Repeat"
                    } else {
                        "Repeat · Never"
                    })
                    .icon(IconName::ChevronDown)
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.advanced = !view.advanced;
                        cx.notify();
                    })),
            );
        if self.advanced {
            let weak = cx.weak_entity();
            content = content.child(
                Button::new("repeat")
                    .label(self.recurrence.label())
                    .dropdown_menu(move |mut menu, _, _| {
                        for rule in [
                            Recurrence::Never,
                            Recurrence::Daily,
                            Recurrence::Weekly,
                            Recurrence::Monthly,
                            Recurrence::EveryDays(3),
                        ] {
                            let weak = weak.clone();
                            let label = rule.label();
                            menu =
                                menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                    let _ = weak.update(cx, |view, cx| {
                                        view.recurrence = rule.clone();
                                        cx.notify();
                                    });
                                }));
                        }
                        menu
                    }),
            );
            if matches!(self.recurrence, Recurrence::EveryDays(_)) {
                content = content.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child("Every")
                        .child(Input::new(&self.days).w(px(70.)))
                        .child("days"),
                );
            }
        }
        if let Some(error) = &self.error {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(rgb(0xc2594e))
                    .child(error.clone()),
            );
        }
        content.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("cancel-reminder")
                        .ghost()
                        .label("Cancel")
                        .on_click(cx.listener(|view, _, window, cx| {
                            if view.inline {
                                view.state.update(cx, |state, cx| {
                                    state.floating_reminder = false;
                                    cx.notify();
                                });
                            } else {
                                window.close_dialog(cx);
                            }
                        })),
                )
                .child(
                    Button::new("save-reminder")
                        .primary()
                        .label("Set reminder")
                        .on_click(cx.listener(|view, _, window, cx| view.save(window, cx))),
                ),
        )
    }
}

pub fn open_picker(state: Entity<AppState>, id: String, window: &mut Window, cx: &mut App) {
    let picker = cx.new(|cx| ReminderPicker::new(state, id, false, window, cx));
    window.open_dialog(cx, move |dialog, _, _| {
        dialog.title("Reminder").w(px(420.)).child(picker.clone())
    });
}

pub fn reminders_page(state: &Entity<AppState>, cx: &App) -> AnyElement {
    let p = palette(cx);
    let app = state.read(cx);
    let today = Local::now().date_naive();
    let mut content = div()
        .id("reminders-page")
        .size_full()
        .v_flex()
        .overflow_y_scroll()
        .px_8()
        .py_6()
        .gap_5()
        .bg(p.paper)
        .child(div().text_size(px(23.)).font_semibold().child("Reminders"));
    for group in ["Today", "Upcoming", "Completed"] {
        let reminders: Vec<_> = app
            .reminders
            .iter()
            .filter(|r| {
                let date = chrono::DateTime::from_timestamp(r.scheduled_at, 0)
                    .map(|d| d.with_timezone(&Local).date_naive());
                match group {
                    "Completed" => r.status == "completed",
                    "Today" => r.status != "completed" && date.is_some_and(|d| d <= today),
                    _ => r.status != "completed" && date.is_some_and(|d| d > today),
                }
            })
            .collect();
        if reminders.is_empty() {
            continue;
        }
        content = content.child(
            div()
                .mt_3()
                .text_xs()
                .font_semibold()
                .text_color(p.muted)
                .child(group.to_uppercase()),
        );
        for r in reminders {
            let open = state.clone();
            let note_id = r.note_id.clone();
            let done = state.clone();
            let done_id = r.id.clone();
            let snooze = state.clone();
            let snooze_id = r.id.clone();
            let remove = state.clone();
            let remove_id = r.id.clone();
            let mut actions = div().flex().gap_1();
            if r.status != "completed" {
                actions = actions
                    .child(
                        Button::new(SharedString::from(format!("done-{}", r.id)))
                            .ghost()
                            .small()
                            .icon(IconName::Check)
                            .tooltip("Mark done")
                            .on_click(move |_, _, cx| {
                                done.update(cx, |state, cx| {
                                    state.perform(
                                        Request::Complete(done_id.clone()),
                                        "Reminder completed",
                                        cx,
                                    )
                                })
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("snooze-{}", r.id)))
                            .ghost()
                            .small()
                            .icon(IconName::Clock)
                            .tooltip("Snooze")
                            .on_click(move |_, _, cx| {
                                snooze.update(cx, |state, cx| {
                                    state.perform(
                                        Request::Snooze(
                                            snooze_id.clone(),
                                            state.settings.snooze_minutes,
                                        ),
                                        "Reminder snoozed",
                                        cx,
                                    )
                                })
                            }),
                    );
            }
            actions = actions.child(
                Button::new(SharedString::from(format!("remove-{}", r.id)))
                    .ghost()
                    .small()
                    .icon(IconName::X)
                    .tooltip("Remove reminder")
                    .on_click(move |_, window, cx| {
                        let state = remove.clone();
                        let id = remove_id.clone();
                        window.open_dialog(cx, move |dialog, _, _| {
                            let state = state.clone();
                            let id = id.clone();
                            dialog
                                .title("Remove reminder?")
                                        .child("This removes the reminder and its repeating occurrences. The note will be kept.")
                                .button_props(
                                    dialog::DialogButtonProps::default()
                                        .ok_text("Remove")
                                        .show_cancel(true),
                                )
                                .on_ok(move |_, _, cx| {
                                    state.update(cx, |state, cx| {
                                        state.perform(
                                            Request::RemoveReminder(id.clone()),
                                            "Reminder removed",
                                            cx,
                                        )
                                    });
                                    true
                                })
                        });
                    }),
            );
            content = content.child(
                div()
                    .flex()
                    .items_center()
                    .gap_5()
                    .pb_4()
                    .border_b_1()
                    .border_color(p.line)
                    .child(
                        div()
                            .w(px(130.))
                            .text_sm()
                            .text_color(p.muted)
                            .child(date_label(r.scheduled_at)),
                    )
                    .child(
                        Button::new(SharedString::from(format!("reminder-note-{}", r.id)))
                            .ghost()
                            .label(r.title.clone())
                            .flex_1()
                            .justify_start()
                            .on_click(move |_, _, cx| {
                                open.update(cx, |state, cx| state.open_note(&note_id, cx))
                            }),
                    )
                    .child(div().text_xs().text_color(p.muted).child(
                        if r.recurrence == Recurrence::Never {
                            String::new()
                        } else {
                            r.recurrence.label()
                        },
                    ))
                    .child(actions),
            );
        }
    }
    if app.reminders.is_empty() {
        content = content.child(
            div()
                .py_8()
                .text_color(p.muted)
                .child("No reminders. Add one from any note."),
        );
    }
    content.into_any_element()
}
