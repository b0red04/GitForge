pub fn rgba_to_hsla(c: gpui::Rgba) -> gpui::Hsla {
    c.into()
}

#[derive(Debug, Clone)]
pub struct AppColors {
    pub background: gpui::Rgba,
    pub surface: gpui::Rgba,
    pub surface_high: gpui::Rgba,
    pub border: gpui::Rgba,
    pub border_focused: gpui::Rgba,
    pub text: gpui::Rgba,
    pub text_muted: gpui::Rgba,
    pub accent: gpui::Rgba,
    pub accent_secondary: gpui::Rgba,
    pub error: gpui::Rgba,
    pub warning: gpui::Rgba,
    pub success: gpui::Rgba,

    pub sidebar_background: gpui::Rgba,
    pub sidebar_text: gpui::Rgba,
    pub sidebar_selected: gpui::Rgba,
    pub sidebar_hover: gpui::Rgba,

    pub commit_hash: gpui::Rgba,
    pub ref_branch: gpui::Rgba,
    pub ref_tag: gpui::Rgba,
    pub ref_remote: gpui::Rgba,
    pub ref_head: gpui::Rgba,

    pub diff_added: gpui::Rgba,
    pub diff_added_bg: gpui::Rgba,
    pub diff_removed: gpui::Rgba,
    pub diff_removed_bg: gpui::Rgba,
    pub diff_hunk_header: gpui::Rgba,

    pub graph_lanes: Vec<gpui::Rgba>,

    pub scroll_bar: gpui::Rgba,
    pub scroll_bar_hover: gpui::Rgba,
    pub selection: gpui::Rgba,
    pub selection_bg: gpui::Rgba,

    pub syntax_keyword: gpui::Rgba,
    pub syntax_function: gpui::Rgba,
    pub syntax_string: gpui::Rgba,
    pub syntax_number: gpui::Rgba,
    pub syntax_comment: gpui::Rgba,
    pub syntax_type: gpui::Rgba,
    pub syntax_variable: gpui::Rgba,
    pub syntax_property: gpui::Rgba,
}

fn parse_hex(hex: &str) -> gpui::Rgba {
    let hex = hex.trim_start_matches('#');
    let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
    gpui::Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    }
}

impl AppColors {
    pub fn from_theme(theme: &crate::Theme) -> Self {
        let c = &theme.colors;

        Self {
            background: parse_hex(&c.background),
            surface: parse_hex(&c.surface),
            surface_high: parse_hex(&c.surface_high),
            border: parse_hex(&c.border),
            border_focused: parse_hex(&c.border_focused),
            text: parse_hex(&c.text),
            text_muted: parse_hex(&c.text_muted),
            accent: parse_hex(&c.accent),
            accent_secondary: parse_hex(&c.accent_secondary),
            error: parse_hex(&c.error),
            warning: parse_hex(&c.warning),
            success: parse_hex(&c.success),

            sidebar_background: parse_hex(&c.sidebar_background),
            sidebar_text: parse_hex(&c.sidebar_text),
            sidebar_selected: parse_hex(&c.sidebar_selected),
            sidebar_hover: parse_hex(&c.sidebar_hover),

            commit_hash: parse_hex(&c.commit_hash),
            ref_branch: parse_hex(&c.ref_branch),
            ref_tag: parse_hex(&c.ref_tag),
            ref_remote: parse_hex(&c.ref_remote),
            ref_head: parse_hex(&c.ref_head),

            diff_added: parse_hex(&c.diff_added),
            diff_added_bg: parse_hex(&c.diff_added_bg),
            diff_removed: parse_hex(&c.diff_removed),
            diff_removed_bg: parse_hex(&c.diff_removed_bg),
            diff_hunk_header: parse_hex(&c.diff_hunk_header),

            graph_lanes: [
                &c.graph_lane_1,
                &c.graph_lane_2,
                &c.graph_lane_3,
                &c.graph_lane_4,
                &c.graph_lane_5,
                &c.graph_lane_6,
                &c.graph_lane_7,
                &c.graph_lane_8,
            ]
            .iter()
            .map(|s| parse_hex(s))
            .collect(),

            scroll_bar: parse_hex(&c.scroll_bar),
            scroll_bar_hover: parse_hex(&c.scroll_bar_hover),
            selection: parse_hex(&c.selection),
            selection_bg: parse_hex(&c.selection_bg),

            syntax_keyword: parse_hex(&c.syntax_keyword),
            syntax_function: parse_hex(&c.syntax_function),
            syntax_string: parse_hex(&c.syntax_string),
            syntax_number: parse_hex(&c.syntax_number),
            syntax_comment: parse_hex(&c.syntax_comment),
            syntax_type: parse_hex(&c.syntax_type),
            syntax_variable: parse_hex(&c.syntax_variable),
            syntax_property: parse_hex(&c.syntax_property),
        }
    }

    pub fn graph_lane_color(&self, lane: usize) -> gpui::Rgba {
        self.graph_lanes[lane % self.graph_lanes.len()]
    }

    pub fn scope_color(&self, scope: &gitforge_syntax::HighlightScope) -> gpui::Rgba {
        use gitforge_syntax::HighlightScope;
        match scope {
            HighlightScope::Keyword => self.syntax_keyword,
            HighlightScope::Function => self.syntax_function,
            HighlightScope::String => self.syntax_string,
            HighlightScope::Number => self.syntax_number,
            HighlightScope::Comment => self.syntax_comment,
            HighlightScope::Type => self.syntax_type,
            HighlightScope::Variable => self.syntax_variable,
            HighlightScope::Property => self.syntax_property,
            HighlightScope::Default => self.text,
        }
    }
}
