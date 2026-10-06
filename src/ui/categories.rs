use crate::{
    app::AppState,
    models::{Category, Collection},
};
use gpui_kit::{
    component::{
        button::*,
        dialog::DialogButtonProps,
        input::{Input, InputState},
        menu::*,
        *,
    },
    prelude::FluentBuilder,
    *,
};

pub fn picker(state: Entity<AppState>, width: f32, cx: &App) -> impl IntoElement {
    let label = state.read(cx).category_label().to_owned();
    let p = crate::theme::palette(cx);
    Button::new("category-picker")
        .ghost()
        .small()
        .w(px(width.max(0.)))
        .min_w_0()
        .h(px(37.))
        .px_4()
        .child(
            div()
                .w_full()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(div().min_w_0().truncate().child(label.clone()))
                .child(
                    Icon::new(IconName::ChevronDown)
                        .size_3()
                        .text_color(p.muted),
                ),
        )
        .accessibility_label(label)
        .rounded(ButtonRounded::None)
        .tooltip("Categories")
        .dropdown_menu(move |menu, _, cx| category_menu(menu, state.clone(), cx))
}

pub fn category_menu(mut menu: PopupMenu, state: Entity<AppState>, cx: &App) -> PopupMenu {
    menu = menu.min_w(px(216.));
    let app = state.read(cx);
    let active = app.session.category_id.clone();
    let collection = app.collection;
    let categories = app.categories.clone();
    for filter in [Collection::All, Collection::Pinned] {
        let state = state.clone();
        menu = menu.item(
            PopupMenuItem::new(filter.label())
                .checked(active.is_none() && collection == filter)
                .on_click(move |_, _, cx| state.update(cx, |state, cx| state.navigate(filter, cx))),
        );
    }
    menu = menu.separator();
    for category in categories {
        let selected = active.as_ref() == Some(&category.id);
        let state = state.clone();
        menu = menu.item(
            PopupMenuItem::new(category.name)
                .checked(selected)
                .on_click(move |_, _, cx| {
                    state.update(cx, |state, cx| state.select_category(&category.id, cx))
                }),
        );
    }
    menu = menu.separator();
    for filter in [Collection::Reminders, Collection::Archive] {
        let state = state.clone();
        menu = menu.item(
            PopupMenuItem::new(filter.label())
                .checked(active.is_none() && collection == filter)
                .on_click(move |_, _, cx| state.update(cx, |state, cx| state.navigate(filter, cx))),
        );
    }
    let create = state.clone();
    menu = menu.separator().item(
        PopupMenuItem::new("New category…")
            .on_click(move |_, window, cx| edit_category(create.clone(), None, window, cx)),
    );
    if let Some(category) = active.and_then(|id| {
        state
            .read(cx)
            .categories
            .iter()
            .find(|c| c.id == id)
            .cloned()
    }) {
        let rename = state.clone();
        let edit = category.clone();
        menu = menu.item(
            PopupMenuItem::new("Rename category…").on_click(move |_, window, cx| {
                edit_category(rename.clone(), Some(edit.clone()), window, cx)
            }),
        );
        menu = menu.item(
            PopupMenuItem::new("Delete category…").on_click(move |_, window, cx| {
                let state = state.clone();
                let id = category.id.clone();
                window.open_alert_dialog(cx, move |dialog, _, _| {
                    let state = state.clone();
                    let id = id.clone();
                    dialog
                        .confirm()
                        .title("Delete category?")
                        .child("Its notes will stay in All Notes.")
                        .button_props(
                            DialogButtonProps::default()
                                .ok_text("Delete category")
                                .ok_variant(ButtonVariant::Danger)
                                .show_cancel(true),
                        )
                        .on_ok(move |_, _, cx| {
                            state.update(cx, |state, cx| state.delete_category(id.clone(), cx));
                            true
                        })
                });
            }),
        );
    }
    menu
}

struct CategoryForm {
    input: Entity<InputState>,
    error: Option<String>,
}
impl Render for CategoryForm {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .v_flex()
            .gap_2()
            .child(Input::new(&self.input).aria_label("Category name"))
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_sm().text_color(rgb(0xf3a6a1)).child(error))
            })
    }
}

fn edit_category(
    state: Entity<AppState>,
    category: Option<Category>,
    window: &mut Window,
    cx: &mut App,
) {
    let title = if category.is_some() {
        "Rename category"
    } else {
        "New category"
    };
    let id = category
        .as_ref()
        .map(|c| c.id.clone())
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let value = category.map(|c| c.name).unwrap_or_default();
    let input = cx.new(|cx| {
        InputState::new(window, cx)
            .placeholder("Category name")
            .default_value(value)
    });
    let form = cx.new(|_| CategoryForm {
        input: input.clone(),
        error: None,
    });
    window.open_alert_dialog(cx, move |dialog, _, _| {
        let state = state.clone();
        let form = form.clone();
        let id = id.clone();
        dialog
            .confirm()
            .title(title)
            .w(px(340.))
            .child(form.clone())
            .button_props(
                DialogButtonProps::default()
                    .ok_text("Done")
                    .show_cancel(true),
            )
            .on_ok(move |_, _, cx| {
                let name = form.read(cx).input.read(cx).value().trim().to_owned();
                let error = if !(1..=48).contains(&name.chars().count()) {
                    Some("Use a name of 1–48 characters.")
                } else if ["all notes", "pinned", "reminders", "archive"]
                    .contains(&name.to_lowercase().as_str())
                {
                    Some("That name is used by a built-in filter.")
                } else if state
                    .read(cx)
                    .categories
                    .iter()
                    .any(|c| c.id != id && c.name.to_lowercase() == name.to_lowercase())
                {
                    Some("A category with that name already exists.")
                } else {
                    None
                };
                if let Some(error) = error {
                    form.update(cx, |form, cx| {
                        form.error = Some(error.into());
                        cx.notify();
                    });
                    return false;
                }
                state.update(cx, |state, cx| state.save_category(id.clone(), name, cx));
                true
            })
    });
    window.defer(cx, move |window, cx| {
        input.update(cx, |input, cx| input.focus(window, cx))
    });
}

pub fn move_note(state: Entity<AppState>, id: String, window: &mut Window, cx: &mut App) {
    window.open_dialog(cx, move |dialog, _, cx| {
        let categories = state.read(cx).categories.clone();
        let mut rows = div().v_flex().gap_1();
        for (category, label) in std::iter::once((None, "No category".to_owned()))
            .chain(categories.into_iter().map(|c| (Some(c.id), c.name)))
        {
            let state = state.clone();
            let id = id.clone();
            rows = rows.child(
                Button::new(SharedString::from(format!(
                    "move-{}",
                    category.as_deref().unwrap_or("none")
                )))
                .ghost()
                .label(label)
                .w_full()
                .justify_start()
                .on_click(move |_, window, cx| {
                    state.update(cx, |state, cx| {
                        state.move_to_category(&id, category.clone(), cx)
                    });
                    window.close_dialog(cx);
                }),
            );
        }
        dialog.title("Move to category").w(px(320.)).child(rows)
    });
}
