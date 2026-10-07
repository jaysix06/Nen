use super::{editor::NoteEditor, lists};
use gpui_kit::base::text::{markdown_ast::Node, *};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{component::*, *};
use std::{ops::Range, path::PathBuf};

fn checkbox(
    editor: Entity<NoteEditor>,
    id: SharedString,
    marker: Range<usize>,
    checked: bool,
    label: String,
    disabled: bool,
    cx: &App,
) -> impl IntoElement {
    let colors = cx.theme();
    let focus = editor.read(cx).checkbox_focus.get(&marker.start).cloned();
    gpui_kit::base::Checkbox::new(id)
        .checked(checked)
        .disabled(disabled)
        .when_some(focus.clone(), |checkbox, focus| {
            checkbox.track_focus(&focus)
        })
        .accessibility_label(if label.is_empty() {
            "Checklist item".into()
        } else {
            label
        })
        .flex()
        .items_center()
        .justify_center()
        .size(px(20.))
        .cursor_pointer()
        .rounded_sm()
        .focus_visible(|style| style.border_1().border_color(colors.primary))
        .child(
            div()
                .size(px(14.))
                .flex()
                .items_center()
                .justify_center()
                .border_1()
                .rounded(px(2.))
                .border_color(colors.foreground)
                .when(checked, |indicator| {
                    indicator
                        .bg(colors.primary)
                        .border_color(colors.primary)
                        .child(
                            Icon::new(IconName::Check)
                                .size(px(12.))
                                .text_color(colors.primary_foreground),
                        )
                }),
        )
        .on_change(move |value, _, window, cx| {
            cx.stop_propagation();
            if let Some(focus) = &focus {
                window.focus(focus, cx);
            }
            editor.update(cx, |view, cx| {
                view.set_checked(
                    marker.clone(),
                    value == gpui_kit::base::CheckboxState::Checked,
                    window,
                    cx,
                )
            });
        })
}

/// Cover only checklist syntax after the textarea lays out. The source editor
/// keeps its native selection, wrapping, scrolling and undo history.
pub fn editing_checkboxes(editor: Entity<NoteEditor>, paper: Hsla) -> impl IntoElement {
    let mut marker_background = paper;
    marker_background.a = 1.;
    canvas(
        move |viewport, window, cx| {
            let (items, disabled) = {
                let view = editor.read(cx);
                let body = view.body.read(cx);
                (
                    lists::checklist_items(&body.value())
                        .into_iter()
                        .filter_map(|item| {
                            body.range_to_bounds(&item.marker)
                                .map(|bounds| (item, bounds))
                        })
                        .collect::<Vec<_>>(),
                    view.state.read(cx).is_quitting() || view.image_busy,
                )
            };
            let mut elements = Vec::new();
            for (item, bounds) in items {
                if !viewport.contains(&bounds.center()) {
                    continue;
                }
                let marker = item.marker.clone();
                let mut element = div()
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .bg(marker_background)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(checkbox(
                        editor.clone(),
                        SharedString::from(format!("edit-check-{}", marker.start)),
                        marker,
                        item.checked,
                        item.label,
                        disabled,
                        cx,
                    ))
                    .into_any_element();
                element.prepaint_as_root(
                    bounds.origin,
                    size(
                        AvailableSpace::Definite(bounds.size.width),
                        AvailableSpace::Definite(bounds.size.height),
                    ),
                    window,
                    cx,
                );
                elements.push(element);
            }
            elements
        },
        |_, elements, window, cx| {
            for mut element in elements {
                element.paint(window, cx);
            }
        },
    )
    .absolute()
    .size_full()
}

#[derive(Clone)]
pub struct NoteDocument {
    pub editor: Entity<NoteEditor>,
    pub directory: PathBuf,
    pub zoom: f32,
    pub available_width: f32,
}

enum Part {
    Task {
        marker: Range<usize>,
        checked: bool,
        label: String,
        indent: usize,
    },
    Markdown {
        text: String,
        offset: usize,
    },
}

impl NoteDocument {
    pub fn markdown(&self, id: impl Into<ElementId>, content: String, cx: &App) -> TextView {
        self.formatted(id, content, true, cx)
    }

    fn formatted(
        &self,
        id: impl Into<ElementId>,
        content: String,
        tasks: bool,
        cx: &App,
    ) -> TextView {
        let directory = self.directory.clone();
        let mut extensions = MarkdownExtensions::default()
            .plugin(NoteImages {
                directory: self.directory.clone(),
                zoom: self.zoom,
                available_width: self.available_width,
                block: true,
            })
            .plugin(NoteImages {
                directory: self.directory.clone(),
                zoom: self.zoom,
                available_width: self.available_width,
                block: false,
            });
        if tasks {
            extensions = extensions.plugin(self.clone());
        }
        TextView::markdown(id, content)
            .markdown_extensions(extensions.parser_revision(if tasks { 1 } else { 2 }))
            .image_source(move |uri| {
                crate::note_images::resolve(uri.as_ref(), &directory)
                    .map(ImageSource::from)
                    .unwrap_or_else(|| ImageSource::from("icons/image-off.svg"))
            })
            .on_link_click(|url, _, _, cx| {
                if url.starts_with("https://")
                    || url.starts_with("http://")
                    || url.starts_with("mailto:")
                {
                    cx.open_url(url);
                }
            })
            .style(TextViewStyle::from_theme(&gpui_kit::base::Theme::global(
                cx,
            )))
    }
}

impl MarkdownPlugin for NoteDocument {
    fn name(&self) -> &str {
        "nen-checklist"
    }
    fn is_block(&self) -> bool {
        true
    }
    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        if !matches!(node, Node::Paragraph(_) | Node::List(_) | Node::Code(_)) {
            return None;
        }
        let source = cx.node_source(node)?;
        let offset = node.position()?.start.offset;
        let document_items = lists::checklist_items(cx.source());
        let items = lists::checklist_items(source)
            .into_iter()
            .filter(|item| {
                document_items.iter().any(|global| {
                    global.marker == (offset + item.marker.start..offset + item.marker.end)
                })
            })
            .collect::<Vec<_>>();
        if items.is_empty() {
            return None;
        }
        if matches!(node, Node::Code(_)) && items.len() != source.lines().count() {
            return None;
        }
        let origin = cx.offset() + node.position()?.start.offset;
        let mut parts = Vec::new();
        let mut offset = 0;
        let mut pending = String::new();
        let mut pending_offset = 0;
        for line in source.split_inclusive('\n') {
            let item = items.iter().find(|item| {
                item.marker.start >= offset && item.marker.start < offset + line.len()
            });
            if let Some(item) = item {
                if !pending.is_empty() {
                    parts.push(Part::Markdown {
                        text: std::mem::take(&mut pending),
                        offset: pending_offset,
                    });
                }
                parts.push(Part::Task {
                    marker: origin + item.marker.start..origin + item.marker.end,
                    checked: item.checked,
                    label: item.label.clone(),
                    indent: item.indent,
                });
            } else {
                if pending.is_empty() {
                    pending_offset = origin + offset;
                }
                pending.push_str(line);
            }
            offset += line.len();
        }
        if !pending.is_empty() {
            parts.push(Part::Markdown {
                text: pending,
                offset: pending_offset,
            });
        }
        Some(
            MarkdownNode::new(self.name(), parts)
                .text(source.to_owned())
                .markdown(source.to_owned()),
        )
    }
    fn render(&self, node: &MarkdownNode, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut block = div().v_flex().w_full().min_w_0();
        let disabled =
            self.editor.read(cx).state.read(cx).is_quitting() || self.editor.read(cx).image_busy;
        for part in node.data::<Vec<Part>>().expect("checklist data") {
            match part {
                Part::Task {
                    marker,
                    checked,
                    label,
                    indent,
                } => {
                    let marker = marker.clone();
                    let start = marker.start;
                    block = block.child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .ml(px(*indent as f32 * 7. * self.zoom))
                            .child(checkbox(
                                self.editor.clone(),
                                SharedString::from(format!("read-check-{start}")),
                                marker,
                                *checked,
                                label.clone(),
                                disabled,
                                cx,
                            ))
                            .child(div().flex_1().min_w_0().child(self.formatted(
                                SharedString::from(format!("task-label-{start}")),
                                label.clone(),
                                false,
                                cx,
                            ))),
                    );
                }
                Part::Markdown { text, offset } => {
                    block = block.child(self.formatted(
                        SharedString::from(format!("task-text-{offset}")),
                        text.clone(),
                        false,
                        cx,
                    ));
                }
            }
        }
        block
    }
}

struct NoteImages {
    directory: PathBuf,
    zoom: f32,
    available_width: f32,
    block: bool,
}
struct ImageData {
    uri: String,
    alt: String,
}
impl MarkdownPlugin for NoteImages {
    fn name(&self) -> &str {
        if self.block {
            "nen-image-block"
        } else {
            "nen-image-inline"
        }
    }
    fn is_block(&self) -> bool {
        self.block
    }
    fn parse(&self, node: &Node, cx: &MarkdownParseContext<'_>) -> Option<MarkdownNode> {
        let image = match node {
            Node::Paragraph(p) if self.block && p.children.len() == 1 => match &p.children[0] {
                Node::Image(image) => image,
                _ => return None,
            },
            Node::Image(image) if !self.block => image,
            _ => return None,
        };
        Some(
            MarkdownNode::new(
                self.name(),
                ImageData {
                    uri: image.url.clone(),
                    alt: image.alt.clone(),
                },
            )
            .text(image.alt.clone())
            .markdown(cx.node_source(node)?.to_owned()),
        )
    }
    fn render(&self, node: &MarkdownNode, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let data = node.data::<ImageData>().expect("image data");
        let mut view = div()
            .id(SharedString::from(format!(
                "note-image-{}",
                node.source_range().map_or(0, |r| r.start)
            )))
            .test_support()
            .flex_shrink_0();
        if let Some(path) = crate::note_images::resolve(&data.uri, &self.directory) {
            let resource = Resource::Path(path.into());
            match window.use_asset::<ImgResourceLoader>(&resource, cx) {
                Some(Ok(image)) => {
                    let width =
                        (image.size(0).width.0 as f32).min(self.available_width) * self.zoom;
                    let height =
                        width * image.size(0).height.0 as f32 / image.size(0).width.0 as f32;
                    view = view.w(px(width)).h(px(height)).child(
                        img(image)
                            .size_full()
                            .object_fit(ObjectFit::Contain)
                            .aria_label(data.alt.clone()),
                    );
                }
                Some(Err(_)) => {
                    view = view.child("Image unavailable");
                }
                None => {
                    view = view.child("Loading image...");
                }
            }
        } else {
            view = view.child("Image unavailable");
        }
        view
    }
}
