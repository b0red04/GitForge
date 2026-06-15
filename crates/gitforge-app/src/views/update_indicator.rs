use gitforge_ui::{AppColors, rgba_to_hsla};
use gitforge_update::{AutoUpdateStatus, AutoUpdater, UpdateCheckType, VersionCheckType};
use gpui::*;

pub struct UpdateIndicator {
  status: AutoUpdateStatus,
  update_check_type: UpdateCheckType,
  dismissed: bool,
  colors: AppColors,
}

impl UpdateIndicator {
  pub fn new(colors: AppColors, cx: &mut Context<Self>) -> Self {
    if let Some(auto_updater) = AutoUpdater::get(cx) {
      cx.observe(&auto_updater, |this, auto_update, cx| {
        this.status = auto_update.read(cx).status();
        this.update_check_type = auto_update.read(cx).update_check_type();
        if this.status.is_updated() {
          this.dismissed = false;
        }
        cx.notify();
      })
      .detach();
      Self {
        status: auto_updater.read(cx).status(),
        update_check_type: UpdateCheckType::Automatic,
        dismissed: false,
        colors,
      }
    } else {
      Self {
        status: AutoUpdateStatus::Idle,
        update_check_type: UpdateCheckType::Automatic,
        dismissed: false,
        colors,
      }
    }
  }

  pub fn set_colors(&mut self, colors: AppColors, cx: &mut Context<Self>) {
    self.colors = colors;
    cx.notify();
  }
}

impl Render for UpdateIndicator {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    if self.dismissed {
      return div().into_any_element();
    }

    let muted = rgba_to_hsla(self.colors.text_muted);
    let accent = rgba_to_hsla(self.colors.accent);
    let hover_bg = rgba_to_hsla(self.colors.surface_high);

    match &self.status {
      AutoUpdateStatus::Checking if self.update_check_type.is_manual() => chip("Checking…", muted)
        .into_any_element(),
      AutoUpdateStatus::Downloading { version } => {
        chip(format!("Downloading {}", version_label(version)), muted).into_any_element()
      }
      AutoUpdateStatus::Installing { version } => {
        chip(format!("Installing {}", version_label(version)), muted).into_any_element()
      }
      AutoUpdateStatus::Updated { version } => button(
        format!("Restart to update to {}", version_label(version)),
        accent,
        hover_bg,
        cx,
        |_, _, cx| {
          cx.restart();
        },
      )
      .into_any_element(),
      AutoUpdateStatus::Errored { error } => button(
        format!("Update failed: {}", error),
        accent,
        hover_bg,
        cx,
        |_, _, cx| {
          if let Some(updater) = AutoUpdater::get(cx) {
            updater.update(cx, |updater, cx| {
              updater.dismiss(cx);
            });
          }
        },
      )
      .into_any_element(),
      AutoUpdateStatus::Idle | AutoUpdateStatus::Checking { .. } => div().into_any_element(),
    }
  }
}

fn version_label(version: &VersionCheckType) -> String {
  match version {
    VersionCheckType::Semantic(version) => version.to_string(),
  }
}

fn chip(label: impl Into<SharedString>, color: Hsla) -> impl IntoElement {
  div()
    .id("update-indicator")
    .px_2()
    .py_0p5()
    .rounded(px(4.0))
    .text_xs()
    .text_color(color)
    .child(label.into())
}

fn button(
  label: impl Into<SharedString>,
  accent: Hsla,
  hover_bg: Hsla,
  cx: &mut Context<UpdateIndicator>,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
  div()
    .id("update-indicator")
    .px_2()
    .py_0p5()
    .rounded(px(4.0))
    .text_xs()
    .text_color(accent)
    .cursor_pointer()
    .hover(|s| s.bg(hover_bg))
    .child(label.into())
    .on_click(on_click)
    .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| {
      this.dismissed = true;
      cx.notify();
    }))
}
