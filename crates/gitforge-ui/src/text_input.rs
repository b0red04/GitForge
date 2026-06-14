use crate::{AppColors, rgba_to_hsla};
use gpui::*;

pub const CURSOR_CHAR: &str = "\u{2502}";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextInputMode {
    #[default]
    SingleLine,
    MultiLine {
        /// When true, suppress the cursor bar when text ends with a newline.
        suppress_cursor_after_newline: bool,
    },
}

impl TextInputMode {
    pub const MULTILINE: Self = Self::MultiLine {
        suppress_cursor_after_newline: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextInputDisplay {
    #[default]
    Plain,
    /// Shows fixed bullets when unfocused and `configured` is true.
    MaskedBullets,
    /// Shows one bullet per character plus cursor when focused.
    MaskedWithCursor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputEvent {
    Typed(String),
    Backspace,
    Enter {
        key_char: Option<String>,
    },
    Escape,
    ArrowUp,
    ArrowDown,
    Unhandled,
}

pub struct TextInput {
    text: String,
    focus_handle: FocusHandle,
    placeholder: SharedString,
    mode: TextInputMode,
    display: TextInputDisplay,
}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut App) -> Self {
        Self {
            text: String::new(),
            focus_handle: cx.focus_handle(),
            placeholder: placeholder.into(),
            mode: TextInputMode::SingleLine,
            display: TextInputDisplay::Plain,
        }
    }

    pub fn with_mode(mut self, mode: TextInputMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_display(mut self, display: TextInputDisplay) -> Self {
        self.display = display;
        self
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn text_mut(&mut self) -> &mut String {
        &mut self.text
    }

    pub fn clear(&mut self) {
        self.text.clear();
    }

    pub fn focus_handle(&self) -> &FocusHandle {
        &self.focus_handle
    }

    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    pub fn mode(&self) -> TextInputMode {
        self.mode
    }

    pub fn display(&self) -> TextInputDisplay {
        self.display
    }

    pub fn is_focused(&self, window: &Window) -> bool {
        self.focus_handle.is_focused(window)
    }

    /// Apply a typed character or backspace (`None`).
    pub fn edit(&mut self, ch: Option<&str>) {
        match ch {
            Some(c) => self.text.push_str(c),
            None => {
                self.text.pop();
            }
        }
    }

    pub fn on_key_down(&mut self, ev: &KeyDownEvent, _window: &Window) -> TextInputEvent {
        let key = ev.keystroke.key.as_str();
        match key {
            "backspace" => TextInputEvent::Backspace,
            "escape" => TextInputEvent::Escape,
            "enter" => TextInputEvent::Enter {
                key_char: ev.keystroke.key_char.clone(),
            },
            "up" => TextInputEvent::ArrowUp,
            "down" => TextInputEvent::ArrowDown,
            _ => {
                if let Some(ch) = typed_character(ev) {
                    TextInputEvent::Typed(ch)
                } else {
                    TextInputEvent::Unhandled
                }
            }
        }
    }

    pub fn display_parts(&self, focused: bool, configured: bool) -> (String, bool) {
        let is_empty = self.text.is_empty();
        let show_placeholder = is_empty && !focused;

        let display = match self.display {
            TextInputDisplay::Plain => {
                if show_placeholder {
                    self.placeholder.to_string()
                } else {
                    let mut t = self.text.clone();
                    if focused && should_show_cursor(self.mode, &self.text) {
                        t.push_str(CURSOR_CHAR);
                    }
                    t
                }
            }
            TextInputDisplay::MaskedBullets => {
                if show_placeholder {
                    self.placeholder.to_string()
                } else if focused {
                    let mut s = self.text.clone();
                    s.push_str(CURSOR_CHAR);
                    s
                } else if configured || !is_empty {
                    "••••••••••••".to_string()
                } else {
                    String::new()
                }
            }
            TextInputDisplay::MaskedWithCursor => {
                if show_placeholder {
                    self.placeholder.to_string()
                } else if focused {
                    let masked = "\u{2022}".repeat(self.text.chars().count());
                    format!("{masked}{CURSOR_CHAR}")
                } else if !is_empty {
                    "••••••••••••".to_string()
                } else {
                    String::new()
                }
            }
        };

        (display, show_placeholder)
    }

    pub fn display_parts_with_placeholder(
        &self,
        focused: bool,
        configured: bool,
        placeholder: &str,
    ) -> (String, bool) {
        let is_empty = self.text.is_empty();
        let show_placeholder = is_empty && !focused;

        let display = match self.display {
            TextInputDisplay::Plain => {
                if show_placeholder {
                    placeholder.to_string()
                } else {
                    let mut t = self.text.clone();
                    if focused && should_show_cursor(self.mode, &self.text) {
                        t.push_str(CURSOR_CHAR);
                    }
                    t
                }
            }
            TextInputDisplay::MaskedBullets => {
                if show_placeholder {
                    placeholder.to_string()
                } else if focused {
                    let mut s = self.text.clone();
                    s.push_str(CURSOR_CHAR);
                    s
                } else if configured || !is_empty {
                    "••••••••••••".to_string()
                } else {
                    String::new()
                }
            }
            TextInputDisplay::MaskedWithCursor => {
                if show_placeholder {
                    placeholder.to_string()
                } else if focused {
                    let masked = "\u{2022}".repeat(self.text.chars().count());
                    format!("{masked}{CURSOR_CHAR}")
                } else if !is_empty {
                    "••••••••••••".to_string()
                } else {
                    String::new()
                }
            }
        };

        (display, show_placeholder)
    }
}

fn should_show_cursor(mode: TextInputMode, text: &str) -> bool {
    match mode {
        TextInputMode::SingleLine => true,
        TextInputMode::MultiLine {
            suppress_cursor_after_newline,
        } => {
            if suppress_cursor_after_newline {
                !text.ends_with('\n')
            } else {
                true
            }
        }
    }
}

pub fn modifier_keys_prevent_typing(modifiers: &Modifiers) -> bool {
    modifiers.control || modifiers.alt || modifiers.platform
}

pub fn typed_character(ev: &KeyDownEvent) -> Option<String> {
    if modifier_keys_prevent_typing(&ev.keystroke.modifiers) {
        return None;
    }
    if let Some(ch) = ev.keystroke.key_char.clone() {
        return Some(ch);
    }
    let key = ev.keystroke.key.as_str();
    if key.len() == 1 {
        return Some(key.to_string());
    }
    None
}

/// Styling options for [`render_text_input`].
pub struct TextInputRenderOpts {
    pub id: ElementId,
    pub configured: bool,
    pub width: Option<Pixels>,
    pub min_h: Option<Pixels>,
    pub max_h: Option<Pixels>,
    pub text_xs: bool,
    pub text_sm: bool,
    pub font_family: Option<&'static str>,
    pub border: bool,
    pub border_bottom: bool,
    pub rounded: bool,
    pub padding_x: Pixels,
    pub padding_y: Pixels,
    pub background: Option<Hsla>,
    pub overflow_hidden: bool,
    pub overflow_y_scroll: bool,
    pub overflow_x_hidden: bool,
    pub text_ellipsis: bool,
    pub flex_1: bool,
    pub flex_shrink_0: bool,
    pub cursor_pointer: bool,
    pub placeholder: Option<String>,
    pub force_focused: Option<bool>,
}

impl TextInputRenderOpts {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            configured: false,
            width: None,
            min_h: None,
            max_h: None,
            text_xs: false,
            text_sm: true,
            font_family: None,
            border: true,
            border_bottom: false,
            rounded: true,
            padding_x: px(8.0),
            padding_y: px(4.0),
            background: None,
            overflow_hidden: false,
            overflow_y_scroll: false,
            overflow_x_hidden: false,
            text_ellipsis: false,
            flex_1: false,
            flex_shrink_0: false,
            cursor_pointer: true,
            placeholder: None,
            force_focused: None,
        }
    }

    pub fn force_focused(mut self, focused: bool) -> Self {
        self.force_focused = Some(focused);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    pub fn configured(mut self, configured: bool) -> Self {
        self.configured = configured;
        self
    }

    pub fn width(mut self, width: Pixels) -> Self {
        self.width = Some(width);
        self
    }

    pub fn min_h(mut self, min_h: Pixels) -> Self {
        self.min_h = Some(min_h);
        self
    }

    pub fn max_h(mut self, max_h: Pixels) -> Self {
        self.max_h = Some(max_h);
        self
    }

    pub fn text_xs(mut self) -> Self {
        self.text_xs = true;
        self.text_sm = false;
        self
    }

    pub fn no_border(mut self) -> Self {
        self.border = false;
        self
    }

    pub fn border_bottom(mut self) -> Self {
        self.border_bottom = true;
        self.border = false;
        self
    }

    pub fn no_rounded(mut self) -> Self {
        self.rounded = false;
        self
    }

    pub fn background(mut self, bg: Hsla) -> Self {
        self.background = Some(bg);
        self
    }

    pub fn font_family(mut self, family: &'static str) -> Self {
        self.font_family = Some(family);
        self
    }

    pub fn overflow_hidden(mut self) -> Self {
        self.overflow_hidden = true;
        self
    }

    pub fn text_ellipsis(mut self) -> Self {
        self.text_ellipsis = true;
        self
    }

    pub fn flex_1(mut self) -> Self {
        self.flex_1 = true;
        self
    }

    pub fn overflow_y_scroll(mut self) -> Self {
        self.overflow_y_scroll = true;
        self
    }

    pub fn overflow_x_hidden(mut self) -> Self {
        self.overflow_x_hidden = true;
        self
    }
}

pub fn render_text_input(
    input: &TextInput,
    colors: &AppColors,
    window: &Window,
    opts: &TextInputRenderOpts,
    on_click: impl Fn(&mut Window) + 'static,
) -> Stateful<Div> {
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let bg = opts
        .background
        .unwrap_or_else(|| rgba_to_hsla(colors.background));

    let focused = opts.force_focused.unwrap_or_else(|| input.is_focused(window));
    let border_color = if focused { accent } else { border };
    let placeholder = opts
        .placeholder
        .as_deref()
        .unwrap_or(input.placeholder());
    let (display_text, show_placeholder) =
        input.display_parts_with_placeholder(focused, opts.configured, placeholder);
    let display_color = if show_placeholder { muted } else { text_color };

    let fh = input.focus_handle().clone();

    let mut outer = div()
        .id(opts.id.clone())
        .track_focus(input.focus_handle())
        .px(opts.padding_x)
        .py(opts.padding_y);

    if opts.cursor_pointer {
        outer = outer.cursor_pointer();
    }
    if let Some(w) = opts.width {
        outer = outer.w(w);
    }
    if let Some(h) = opts.min_h {
        outer = outer.min_h(h);
    }
    if let Some(h) = opts.max_h {
        outer = outer.max_h(h);
    }
    if opts.flex_1 {
        outer = outer.flex_1().min_h(px(0.0));
    }
    if opts.flex_shrink_0 {
        outer = outer.flex_shrink_0();
    }
    if opts.border {
        outer = outer.border_1().border_color(border_color);
        if opts.rounded {
            outer = outer.rounded(px(3.0));
        }
    } else if opts.border_bottom {
        outer = outer.border_b_1().border_color(border);
    }
    if opts.rounded && !opts.border {
        outer = outer.rounded(px(4.0));
    }
    if opts.overflow_y_scroll {
        outer = outer.overflow_y_scroll();
    }
    if opts.overflow_x_hidden {
        outer = outer.overflow_x_hidden();
    }

    outer = outer.bg(bg).on_click(move |_ev, window, _cx| {
        window.focus(&fh);
        on_click(window);
    });

    let mut text_div = div().text_color(display_color);
    if opts.text_xs {
        text_div = text_div.text_xs();
    } else if opts.text_sm {
        text_div = text_div.text_sm();
    }
    if let Some(family) = opts.font_family {
        text_div = text_div.font_family(family);
    }
    if opts.overflow_hidden {
        text_div = text_div.overflow_hidden();
    }
    if opts.text_ellipsis {
        text_div = text_div.text_ellipsis();
    }

    outer.child(text_div.child(display_text))
}

/// Render a text field whose value is owned elsewhere (e.g. settings draft fields).
pub fn render_static_text_input(
    value: &str,
    placeholder: &str,
    focus_handle: &FocusHandle,
    show_cursor: bool,
    display: TextInputDisplay,
    configured: bool,
    colors: &AppColors,
    opts: TextInputRenderOpts,
    on_click: impl Fn(&mut Window) + 'static,
) -> Stateful<Div> {
    let temp = TextInput {
        text: value.to_string(),
        focus_handle: focus_handle.clone(),
        placeholder: placeholder.to_string().into(),
        mode: TextInputMode::SingleLine,
        display,
    };
    let opts = opts
        .placeholder(placeholder)
        .configured(configured)
        .force_focused(show_cursor);
    let border = rgba_to_hsla(colors.border);
    let muted = rgba_to_hsla(colors.text_muted);
    let text_color = rgba_to_hsla(colors.text);
    let accent = rgba_to_hsla(colors.accent);
    let bg = opts
        .background
        .unwrap_or_else(|| rgba_to_hsla(colors.background));

    let border_color = if show_cursor { accent } else { border };
    let placeholder_text = opts
        .placeholder
        .as_deref()
        .unwrap_or(placeholder);
    let (display_text, show_placeholder) =
        temp.display_parts_with_placeholder(show_cursor, opts.configured, placeholder_text);
    let display_color = if show_placeholder { muted } else { text_color };

    let fh = focus_handle.clone();

    let mut outer = div()
        .id(opts.id.clone())
        .track_focus(focus_handle)
        .px(opts.padding_x)
        .py(opts.padding_y);

    if opts.cursor_pointer {
        outer = outer.cursor_pointer();
    }
    if let Some(w) = opts.width {
        outer = outer.w(w);
    }
    if let Some(h) = opts.min_h {
        outer = outer.min_h(h);
    }
    if let Some(h) = opts.max_h {
        outer = outer.max_h(h);
    }
    if opts.flex_1 {
        outer = outer.flex_1().min_h(px(0.0));
    }
    if opts.flex_shrink_0 {
        outer = outer.flex_shrink_0();
    }
    if opts.border {
        outer = outer.border_1().border_color(border_color);
        if opts.rounded {
            outer = outer.rounded(px(3.0));
        }
    } else if opts.border_bottom {
        outer = outer.border_b_1().border_color(border);
    }
    if opts.rounded && !opts.border {
        outer = outer.rounded(px(4.0));
    }
    if opts.overflow_y_scroll {
        outer = outer.overflow_y_scroll();
    }
    if opts.overflow_x_hidden {
        outer = outer.overflow_x_hidden();
    }
    outer = outer.bg(bg).on_click(move |_ev, window, _cx| {
        window.focus(&fh);
        on_click(window);
    });

    let mut text_div = div().text_color(display_color);
    if opts.text_xs {
        text_div = text_div.text_xs();
    } else if opts.text_sm {
        text_div = text_div.text_sm();
    }
    if let Some(family) = opts.font_family {
        text_div = text_div.font_family(family);
    }
    if opts.overflow_hidden {
        text_div = text_div.overflow_hidden();
    }
    if opts.text_ellipsis {
        text_div = text_div.text_ellipsis();
    }

    outer.child(text_div.child(display_text))
}

/// Parse a key event without mutating input state.
pub fn parse_key_event(ev: &KeyDownEvent) -> TextInputEvent {
    let key = ev.keystroke.key.as_str();
    match key {
        "backspace" => TextInputEvent::Backspace,
        "escape" => TextInputEvent::Escape,
        "enter" => TextInputEvent::Enter {
            key_char: ev.keystroke.key_char.clone(),
        },
        "up" => TextInputEvent::ArrowUp,
        "down" => TextInputEvent::ArrowDown,
        _ => {
            if let Some(ch) = typed_character(ev) {
                TextInputEvent::Typed(ch)
            } else {
                TextInputEvent::Unhandled
            }
        }
    }
}

