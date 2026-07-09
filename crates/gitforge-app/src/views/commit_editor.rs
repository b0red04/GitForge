use gitforge_ui::{
    AppColors, HeaderBorder, HeaderPadding, TextInput, TextInputEvent, TextInputMode,
    TextInputRenderOpts, WidgetColors, parse_key_event, render_text_input, rgba_to_hsla,
    section_header,
};
use gpui::*;

pub struct CommitEditor {
    message_input: TextInput,
    ai_alternatives: Vec<String>,
}

impl CommitEditor {
    pub fn new(cx: &mut App) -> Self {
        Self {
            message_input: TextInput::new("Enter commit message...", cx)
                .with_mode(TextInputMode::MULTILINE),
            ai_alternatives: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn message(&self) -> &str {
        self.message_input.text()
    }

    pub fn set_message(&mut self, msg: &str) {
        self.message_input.set_text(msg);
    }

    pub fn type_char(&mut self, ch: &str) {
        self.message_input.edit(Some(ch));
    }

    pub fn backspace(&mut self) {
        self.message_input.edit(None);
    }

    pub fn take_message(&mut self) -> String {
        let msg = self.message_input.text().to_string();
        self.message_input.clear();
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
        let msg = self.ai_alternatives.get(idx).cloned();
        if let Some(msg) = msg {
            self.set_message(&msg);
        }
    }

    pub fn snapshot_data(&self) -> (String, Vec<String>) {
        (
            self.message_input.text().to_string(),
            self.ai_alternatives.clone(),
        )
    }

    pub fn restore_from_snapshot(&mut self, message: String, alternatives: Vec<String>) {
        self.message_input.set_text(message);
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

        let ent1 = entity.clone();
        let ent4 = entity.clone();
        let ent_commit_push = entity.clone();

        let generate_label = if ai_generating {
            "Generating..."
        } else {
            "Generate"
        };
        let generate_color = if ai_generating { muted } else { accent };

        let mut editor = div()
            .id("commit-editor-panel")
            .bg(surface)
            .flex()
            .flex_col();
        if compact {
            editor = editor.flex_shrink_0().border_t_1().border_color(border);
        } else {
            editor = editor.flex_1().h_full();
        }
        editor = editor.child(section_header(
            "commit-editor-header",
            "COMMIT",
            HeaderPadding::Loose,
            HeaderBorder::Bottom,
            None,
            WidgetColors::from_app(colors),
        ));

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
                let is_selected = self.message_input.text() == alt.as_str();
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

        let min_h = px(if compact { 80.0 } else { 120.0 });
        let mut msg_opts = TextInputRenderOpts::new(ElementId::Name("commit-msg-input".into()))
            .min_h(min_h)
            .font_family("monospace")
            .background(bg)
            .no_rounded();
        if compact {
            msg_opts = msg_opts.max_h(px(160.0)).overflow_x_hidden();
        } else {
            msg_opts = msg_opts.flex_1();
        }

        let mut msg_input = div().m_3().p_2().child(
            render_text_input(&self.message_input, colors, window, &msg_opts, |_| {})
                .overflow_y_scroll()
                .on_key_down(move |ev, _window, cx| {
                    if let Some(e) = ent1.upgrade() {
                        e.update(cx, |this, cx| match parse_key_event(ev) {
                            TextInputEvent::Backspace => this.edit_commit_message(None, cx),
                            TextInputEvent::Enter { key_char } => {
                                if let Some(c) = key_char {
                                    this.edit_commit_message(Some(&c), cx);
                                } else {
                                    this.edit_commit_message(Some("\n"), cx);
                                }
                            }
                            TextInputEvent::Escape => this.cancel_commit_dialog(cx),
                            TextInputEvent::Typed(c) => this.edit_commit_message(Some(&c), cx),
                            _ => {}
                        });
                    }
                }),
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
                .justify_between()
                .items_center()
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
                    let ent_cp = ent_commit_push.clone();
                    div()
                        .id("commit-push-btn")
                        .px_3()
                        .py_1()
                        .rounded(px(4.0))
                        .border_1()
                        .border_color(accent)
                        .cursor_pointer()
                        .text_xs()
                        .text_color(accent)
                        .child("Commit & Push")
                        .on_click(move |_ev, _window, cx| {
                            if let Some(e) = ent_cp.upgrade() {
                                e.update(cx, |this, cx| {
                                    this.start_commit_and_push(cx);
                                });
                            }
                        })
                }),
        );
        editor
    }
}
