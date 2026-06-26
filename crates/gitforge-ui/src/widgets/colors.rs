use crate::{AppColors, rgba_to_hsla};
use gpui::*;

#[derive(Clone, Copy)]
pub struct WidgetColors {
    pub surface: Hsla,
    pub surface_high: Hsla,
    pub background: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub muted: Hsla,
    pub accent: Hsla,
    pub warning: Hsla,
    pub sidebar_background: Hsla,
    pub sidebar_hover: Hsla,
    pub sidebar_selected: Hsla,
    pub diff_removed: Hsla,
}

impl WidgetColors {
    pub fn from_app(colors: &AppColors) -> Self {
        Self {
            surface: rgba_to_hsla(colors.surface),
            surface_high: rgba_to_hsla(colors.surface_high),
            background: rgba_to_hsla(colors.background),
            border: rgba_to_hsla(colors.border),
            text: rgba_to_hsla(colors.text),
            muted: rgba_to_hsla(colors.text_muted),
            accent: rgba_to_hsla(colors.accent),
            warning: rgba_to_hsla(colors.warning),
            sidebar_background: rgba_to_hsla(colors.sidebar_background),
            sidebar_hover: rgba_to_hsla(colors.sidebar_hover),
            sidebar_selected: rgba_to_hsla(colors.sidebar_selected),
            diff_removed: rgba_to_hsla(colors.diff_removed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WidgetColors;
    use crate::{AppColors, rgba_to_hsla};

    fn sample_colors() -> AppColors {
        AppColors {
            background: gpui::Rgba {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            surface: gpui::Rgba {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            },
            surface_high: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            },
            border: gpui::Rgba {
                r: 0.1,
                g: 0.1,
                b: 0.1,
                a: 1.0,
            },
            text: gpui::Rgba {
                r: 0.2,
                g: 0.2,
                b: 0.2,
                a: 1.0,
            },
            text_muted: gpui::Rgba {
                r: 0.3,
                g: 0.3,
                b: 0.3,
                a: 1.0,
            },
            accent: gpui::Rgba {
                r: 0.4,
                g: 0.4,
                b: 0.4,
                a: 1.0,
            },
            accent_secondary: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            warning: gpui::Rgba {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            },
            success: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            error: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            border_focused: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            sidebar_background: gpui::Rgba {
                r: 0.6,
                g: 0.6,
                b: 0.6,
                a: 1.0,
            },
            sidebar_text: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            sidebar_selected: gpui::Rgba {
                r: 0.7,
                g: 0.7,
                b: 0.7,
                a: 1.0,
            },
            sidebar_hover: gpui::Rgba {
                r: 0.8,
                g: 0.8,
                b: 0.8,
                a: 1.0,
            },
            commit_hash: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            ref_branch: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            ref_tag: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            ref_remote: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            ref_head: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            diff_added: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            diff_added_bg: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            diff_removed: gpui::Rgba {
                r: 0.9,
                g: 0.9,
                b: 0.9,
                a: 1.0,
            },
            diff_removed_bg: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            diff_hunk_header: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            graph_lanes: vec![],
            scroll_bar: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            scroll_bar_hover: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            selection: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            selection_bg: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_keyword: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_function: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_string: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_number: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_comment: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_type: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_variable: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            syntax_property: gpui::Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }

    #[test]
    fn from_app_maps_every_field() {
        let app = sample_colors();
        let wc = WidgetColors::from_app(&app);
        assert_eq!(wc.surface, rgba_to_hsla(app.surface));
        assert_eq!(wc.surface_high, rgba_to_hsla(app.surface_high));
        assert_eq!(wc.background, rgba_to_hsla(app.background));
        assert_eq!(wc.border, rgba_to_hsla(app.border));
        assert_eq!(wc.text, rgba_to_hsla(app.text));
        assert_eq!(wc.muted, rgba_to_hsla(app.text_muted));
        assert_eq!(wc.accent, rgba_to_hsla(app.accent));
        assert_eq!(wc.warning, rgba_to_hsla(app.warning));
        assert_eq!(wc.sidebar_background, rgba_to_hsla(app.sidebar_background));
        assert_eq!(wc.sidebar_hover, rgba_to_hsla(app.sidebar_hover));
        assert_eq!(wc.sidebar_selected, rgba_to_hsla(app.sidebar_selected));
        assert_eq!(wc.diff_removed, rgba_to_hsla(app.diff_removed));
    }

    #[test]
    fn bundle_is_copy() {
        let app = sample_colors();
        let wc = WidgetColors::from_app(&app);
        let wc2 = wc;
        assert_eq!(wc.surface, wc2.surface);
    }
}
