use gitforge_ui::{rgba_to_hsla, AppColors};
use gpui::*;

pub struct CommitEditor {
    message: String,
    focus_handle: FocusHandle,
    ai_alternatives: Vec<String>,
}

impl CommitEditor {
    pub fn new(cx: &mut App) -> Self {
        Self {
            message: String::new(),
            focus_handle: cx.focus_handle(),
            ai_alternatives: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message.clear();
        self.message.push_str(msg);
    }

    pub fn type_char(&mut self, ch: &str) {
        self.message.push_str(ch);
    }

    pub fn backspace(&mut self) {
        self.message.pop();
    }

    pub fn take_message(&mut self) -> String {
        let msg = self.message.clone();
        self.message.clear();
        self.ai_alternatives.clear();
        msg
    }

    pub fn set_ai_alternatives(&mut self, alts: Vec<String>) {
        self.ai_alternatives = alts;
    }

    #[allow(dead_code)]
    pub fn ai_alternatives(&self) -> &[String] {
        &self.ai_alternatives
    }

    pub fn accept_ai_suggestion(&mut self, idx: usize) {
        let msg = self.ai_alternatives.get(idx).map(String::clone);
        if let Some(msg) = msg {
            self.set_message(&msg);
        }
    }

    pub fn snapshot_data(&self) -> (String, Vec<String>) {
        (self.message.clone(), self.ai_alternatives.clone())
    }

    pub fn restore_from_snapshot(&mut self, message: String, alternatives: Vec<String>) {
        self.message = message;
        self.ai_alternatives = alternatives;
    }

    pub fn render(
        &self,
        colors: &AppColors,
        entity: WeakEntity<super::app::GitForgeApp>,
        window: &mut Window,
        ai_generating: bool,
        compact: bool,
    ) -> Stateful<Div> {
        let surface = rgba_to_hsla(colors.surface);
        let border = rgba_to_hsla(colors.border);
        let muted = rgba_to_hsla(colors.text_muted);
        let text_color = rgba_to_hsla(colors.text);
        let accent = rgba_to_hsla(colors.accent);
        let bg = rgba_to_hsla(colors.background);

        let is_focused = self.focus_handle.is_focused(window);
        let display_text = if self.message.is_empty() && !is_focused {
            String::from("Enter commit message...")
        } else {
            let mut t = self.message.clone();
            if is_focused && !t.ends_with('\n') {
                t.push('\u{2502}');
            }
            t
        };
        let display_color = if self.message.is_empty() && !is_focused {
            muted
        } else {
            text_color
        };
        let border_color = if is_focused { accent } else { border };
        let fh = self.focus_handle.clone();

        let ent1 = entity.clone();
        let ent2 = entity.clone();
        let ent3 = entity.clone();
        let ent4 = entity.clone();
        let has_message = !self.message.trim().is_empty();

        let generate_label = if ai_generating {
            "Generating..."
        } else {
            "Generate"
        };
        let generate_color = if ai_generating { muted } else { accent };

        let mut editor = div().id("commit-editor-panel").bg(surface).flex().flex_col();
        if compact {
            editor = editor.flex_shrink_0().border_t_1().border_color(border);
        } else {
            editor = editor.flex_1().h_full();
        }
        editor = editor.child(
            div()
                .px_3()
                .py_2()
                .border_b_1()
                .border_color(border)
                .text_xs()
                .font_weight(FontWeight::BOLD)
                .text_color(muted)
                .child("COMMIT"),
        );

        if self.ai_alternatives.len() > 1 {
            let mut alt_row = div()
                .px_3()
                .py_1()
                .border_b_1()
                .border_color(border)
                .flex()
                .flex_wrap()
                .gap_1();
            for (i, alt) in self.ai_alternatives.iter().enumerate() {
                let ent_alt = entity.clone();
                let first_line = alt.lines().next().unwrap_or(alt);
                let label = if first_line.len() > 40 {
                    format!("{}...", &first_line[..37])
                } else {
                    first_line.to_string()
                };
                let is_selected = self.message.lines().next().unwrap_or("")
                    == alt.lines().next().unwrap_or("");
                let pill_bg = if is_selected { accent } else { surface };
                let pill_tc = if is_selected {
                    rgba_to_hsla(colors.background)
                } else {
                    text_color
                };
                let pill_bc = if is_selected { accent } else { border };
                alt_row = alt_row.child(
                    div()
                        .id(ElementId::Name(format!("ai-alt-{}", i).into()))
                        .px_2()
                        .py(px(1.0))
                        .border_1()
                        .border_color(pill_bc)
                        .rounded(px(3.0))
                        .bg(pill_bg)
                        .cursor_pointer()
                        .text_xs()
                        .text_color(pill_tc)
                        .child(label)
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_alt.upgrade() {
                                let idx = i;
                                e.update(cx, |this, cx| {
                                    this.select_ai_alternative(idx, cx);
                                });
                            }
                        }),
                );
            }
            editor = editor.child(alt_row);
        }

        let mut msg_input = div()
            .id("commit-msg-input")
            .track_focus(&self.focus_handle)
            .m_3()
            .p_2()
            .min_h(px(if compact { 80.0 } else { 120.0 }))
            .overflow_y_scroll()
            .border_1()
            .border_color(border_color)
            .rounded(px(4.0))
            .bg(bg)
            .on_click(move |_ev, window, _cx| {
                window.focus(&fh);
            })
            .on_key_down(move |ev: &KeyDownEvent, _window, cx| {
                let key = &ev.keystroke.key;
                match key.as_str() {
                    "backspace" => {
                        if let Some(e) = ent1.upgrade() {
                            e.update(cx, |this, cx| {
                                this.edit_commit_message(None, cx);
                            });
                        }
                    }
                    "enter" => {
                        if let Some(e) = ent1.upgrade() {
                            let ch = ev.keystroke.key_char.clone();
                            e.update(cx, |this, cx| {
                                if let Some(c) = ch {
                                    this.edit_commit_message(Some(&c), cx);
                                } else {
                                    this.edit_commit_message(Some("\n"), cx);
                                }
                            });
                        }
                    }
                    "escape" => {
                        if let Some(e) = ent1.upgrade() {
                            e.update(cx, |this, cx| {
                                this.cancel_commit_dialog(cx);
                            });
                        }
                    }
                    _ => {
                        let ch = ev.keystroke.key_char.clone();
                        if let Some(typed) = ch {
                            if !ev.keystroke.modifiers.platform {
                                if let Some(e) = ent1.upgrade() {
                                    let c = typed;
                                    e.update(cx, |this, cx| {
                                        this.edit_commit_message(Some(&c), cx);
                                    });
                                }
                            }
                        }
                    }
                }
            })
            .child(
                div()
                    .text_sm()
                    .font_family("monospace")
                    .text_color(display_color)
                    .child(display_text),
            );
        if compact {
            msg_input = msg_input.max_h(px(160.0)).overflow_x_hidden();
        } else {
            msg_input = msg_input.flex_1().min_h(px(0.0));
        }
        editor = editor.child(msg_input);
        editor = editor.child(
                div()
                    .px_3()
                    .py_2()
                    .border_t_1()
                    .border_color(border)
                    .flex()
                    .flex_shrink_0()
                    .gap_2()
                    .child({
                        let ent_gen = ent4.clone();
                        div()
                            .id("ai-generate-btn")
                            .px_3()
                            .py_1()
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(generate_color)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(generate_color)
                            .child(generate_label)
                            .on_click(move |_ev, _window, cx| {
                                if let Some(e) = ent_gen.upgrade() {
                                    e.update(cx, |this, cx| {
                                        this.generate_commit_message(cx);
                                    });
                                }
                            })
                    })
                    .child({
                        let btn_bg = if has_message { accent } else { muted };
                        div()
                            .id("submit-commit-btn")
                            .px_4()
                            .py_1()
                            .rounded(px(4.0))
                            .bg(btn_bg)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(rgba_to_hsla(colors.background))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Commit")
                            .on_click(move |_ev, _window, cx| {
                                if has_message {
                                    if let Some(e) = ent2.upgrade() {
                                        e.update(cx, |this, cx| {
                                            this.perform_commit(false, cx);
                                        });
                                    }
                                }
                            })
                    })
                    .child({
                        div()
                            .id("amend-commit-btn")
                            .px_4()
                            .py_1()
                            .rounded(px(4.0))
                            .border_1()
                            .border_color(border)
                            .cursor_pointer()
                            .text_xs()
                            .text_color(text_color)
                            .child("Amend")
                            .on_click(move |_ev, _window, cx| {
                                if has_message {
                                    if let Some(e) = ent3.upgrade() {
                                        e.update(cx, |this, cx| {
                                            this.perform_commit(true, cx);
                                        });
                                    }
                                }
                            })
                    }),
            );

        editor
    }
}
