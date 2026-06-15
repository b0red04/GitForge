use gpui::*;

#[derive(Clone, Copy)]
pub enum ShellWidth {
    Full,
    Fixed(Pixels),
    Flexible { min_w: Pixels },
}

pub fn panel_shell(width: ShellWidth, bg: Hsla, right_border: bool, relative: bool) -> Div {
    let mut shell = div().h_full().bg(bg).flex().flex_col();

    shell = match width {
        ShellWidth::Full => shell.w_full(),
        ShellWidth::Fixed(w) => shell.w(w),
        ShellWidth::Flexible { min_w } => shell.w_full().min_w(min_w),
    };

    if relative {
        shell = shell.relative();
    }

    if right_border {
        shell = shell.border_r_1();
    }

    shell
}
