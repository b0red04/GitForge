use gitforge_ui::{AppColors, rgba_to_hsla};
use gpui::*;

use super::commands::{CommandEntry, command_palette_entries};

pub struct CommandPalette {
    query: String,
    entries: Vec<CommandEntry>,
    filtered: Vec<usize>,
    selected: usize,
    visible: bool,
    focus_handle: FocusHandle,
}

impl CommandPalette {
    pub fn new(cx: &mut App) -> Self {
        let entries = Self::build_entries();
        let filtered: Vec<usize> = (0..entries.len()).collect();
        Self {
            query: String::new(),
            entries,
            filtered,
            selected: 0,
            visible: false,
            focus_handle: cx.focus_handle(),
        }
    }

    fn build_entries() -> Vec<CommandEntry> {
        command_palette_entries()
    }

    pub fn show(&mut self, _cx: &mut Context<super::app::GitForgeApp>) {
        self.visible = true;
        self.query.clear();
        self.filtered = (0..self.entries.len()).collect();
        self.selected = 0;
    }

    pub fn hide(&mut self, cx: &mut Context<super::app::GitForgeApp>) {
        self.visible = false;
        cx.notify();
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn selected_action(&self) -> Option<&str> {
        let idx = self.filtered.get(self.selected)?;
        Some(&self.entries[*idx].action)
    }

    pub fn on_input(&mut self, text: &str, cx: &mut Context<super::app::GitForgeApp>) {
        self.query.push_str(text);
        self.update_filter();
        self.selected = 0;
        cx.notify();
    }

    pub fn on_backspace(&mut self, cx: &mut Context<super::app::GitForgeApp>) {
        self.query.pop();
        self.update_filter();
        self.selected = 0;
        cx.notify();
    }

    pub fn select_prev(&mut self, cx: &mut Context<super::app::GitForgeApp>) {
        if self.selected > 0 {
            self.selected -= 1;
            cx.notify();
        }
    }

    pub fn select_next(&mut self, cx: &mut Context<super::app::GitForgeApp>) {
        if self.selected + 1 < self.filtered.len() {
            self.selected += 1;
            cx.notify();
        }
    }

    fn update_filter(&mut self) {
        let query = self.query.to_lowercase();
        self.filtered = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                if query.is_empty() {
                    return true;
                }
                let label_lower = e.label.to_lowercase();
                fuzzy_match(&query, &label_lower)
            })
            .map(|(i, _)| i)
            .collect();
    }

    pub fn render(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        window: &mut Window,
    ) -> Option<Stateful<Div>> {
        if !self.visible {
            return None;
        }

        let overlay_bg = rgba_to_hsla(colors.background).opacity(0.6);
        let surface = rgba_to_hsla(colors.surface);
        let border = rgba_to_hsla(colors.border);
        let text_color = rgba_to_hsla(colors.text);
        let muted = rgba_to_hsla(colors.text_muted);
        let accent = rgba_to_hsla(colors.accent);
        let selection_bg = rgba_to_hsla(colors.selection_bg);
        let is_focused = self.focus_handle.is_focused(window);

        let mut display_query = self.query.clone();
        if is_focused {
            display_query.push('\u{2502}');
        }

        let query_color = if self.query.is_empty() && !is_focused {
            muted
        } else {
            text_color
        };

        let placeholder = if self.query.is_empty() && !is_focused {
            "Type a command..."
        } else {
            ""
        };

        let display_text = if self.query.is_empty() && !is_focused {
            placeholder.to_string()
        } else {
            display_query
        };

        let max_visible = 10.min(self.filtered.len());
        let ent_close = entity.clone();
        let ent_key = entity.clone();
        let _ent_up = entity.clone();
        let _ent_down = entity.clone();
        let _ent_enter = entity.clone();

        let mut items = div().flex().flex_col().overflow_hidden().max_h(px(300.0));
        for (i, &entry_idx) in self.filtered.iter().enumerate() {
            if i >= max_visible {
                break;
            }
            let entry = &self.entries[entry_idx];
            let is_selected = i == self.selected;
            let item_bg = if is_selected { selection_bg } else { surface };
            let item_text = if is_selected { accent } else { text_color };
            let label = entry.label;
            let key_hint = entry.keybinding.unwrap_or_default();

            let ent_item = entity.clone();
            let action = entry.action;

            items = items.child(
                div()
                    .id(ElementId::Name(format!("cmd-{}", i).into()))
                    .w_full()
                    .px_3()
                    .py_1()
                    .flex()
                    .items_center()
                    .bg(item_bg)
                    .cursor_pointer()
                    .hover(move |s| s.bg(selection_bg))
                    .on_click(move |_ev, _window, cx| {
                        if let Some(e) = ent_item.upgrade() {
                            e.update(cx, |app, cx| {
                                app.execute_command_palette_action(action, cx);
                            });
                        }
                    })
                    .child(div().flex_1().text_sm().text_color(item_text).child(label))
                    .child(div().text_xs().text_color(muted).child(key_hint)),
            );
        }

        if self.filtered.is_empty() {
            items = items.child(
                div()
                    .px_3()
                    .py_2()
                    .text_sm()
                    .text_color(muted)
                    .child("No matching commands"),
            );
        }

        let focus_handle = self.focus_handle.clone();

        Some(
            div()
                .id("command-palette-overlay")
                .absolute()
                .top_0()
                .left_0()
                .w_full()
                .h_full()
                .bg(overlay_bg)
                .flex()
                .flex_col()
                .items_center()
                .pt(px(80.0))
                .on_click(move |_ev, _window, cx| {
                    if let Some(e) = ent_close.upgrade() {
                        e.update(cx, |app, cx| {
                            app.command_palette.hide(cx);
                        });
                    }
                })
                .child(
                    div()
                        .id("command-palette-box")
                        .w(px(480.0))
                        .bg(surface)
                        .border_1()
                        .border_color(border)
                        .rounded(px(6.0))
                        .shadow(vec![BoxShadow {
                            color: black(),
                            offset: point(px(0.0), px(4.0)),
                            blur_radius: px(12.0),
                            spread_radius: px(0.0),
                        }])
                        .on_click(|_ev, _window, _cx| {})
                        .track_focus(&focus_handle)
                        .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                            if let Some(e) = ent_key.upgrade() {
                                let key = &ev.keystroke.key;
                                let mods = &ev.keystroke.modifiers;
                                if *key == "escape" {
                                    e.update(cx, |app, cx| {
                                        app.command_palette.hide(cx);
                                    });
                                } else if *key == "up" || (mods.platform && *key == "p") {
                                    e.update(cx, |app, cx| {
                                        app.command_palette.select_prev(cx);
                                    });
                                } else if *key == "down" || (mods.platform && *key == "n") {
                                    e.update(cx, |app, cx| {
                                        app.command_palette.select_next(cx);
                                    });
                                } else if *key == "enter" {
                                    e.update(cx, |app, cx| {
                                        app.execute_command_palette_selection(cx);
                                    });
                                } else if *key == "backspace" {
                                    e.update(cx, |app, cx| {
                                        app.command_palette.on_backspace(cx);
                                    });
                                } else if !key.is_empty()
                                    && key.len() == 1
                                    && !mods.platform
                                    && !mods.control
                                {
                                    let ch = key.chars().next().unwrap();
                                    e.update(cx, |app, cx| {
                                        app.command_palette.on_input(&ch.to_string(), cx);
                                    });
                                }
                            }
                        })
                        .child(
                            div()
                                .w_full()
                                .px_3()
                                .py_2()
                                .border_b_1()
                                .border_color(border)
                                .text_sm()
                                .text_color(query_color)
                                .child(display_text),
                        )
                        .child(items),
                ),
        )
    }
}

fn fuzzy_match(query: &str, text: &str) -> bool {
    let mut query_chars = query.chars().peekable();
    for ch in text.chars() {
        if query_chars.peek() == Some(&ch) {
            query_chars.next();
        }
    }
    query_chars.peek().is_none()
}
