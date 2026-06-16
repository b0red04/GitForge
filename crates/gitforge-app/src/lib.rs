pub mod assets;
pub mod views;

use gpui::*;
use tracing_subscriber::EnvFilter;
use views::GitForgeApp;

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("GitForge starting...");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");
    let _guard = rt.enter();

    Application::new()
        .with_assets(assets::EmbeddedAssets::new())
        .run(|cx: &mut App| {
            gitforge_update::init(cx);

            cx.bind_keys([
                KeyBinding::new("ctrl-o", views::OpenRepository, None),
                KeyBinding::new("ctrl-t", views::NewTab, None),
                KeyBinding::new("ctrl-w", views::CloseTab, None),
                KeyBinding::new("ctrl-i", views::InitRepo, None),
                KeyBinding::new("ctrl-shift-e", views::OpenInEditor, None),
                KeyBinding::new("alt-t", views::OpenInTerminal, None),
                KeyBinding::new("alt-o", views::OpenInFileManager, None),
                KeyBinding::new("alt-ctrl-o", views::OpenRepoManagement, None),
                KeyBinding::new("ctrl-comma", views::Preferences, None),
                KeyBinding::new("ctrl-q", views::QuitApp, None),
                KeyBinding::new("escape", views::CloseDialog, None),
                KeyBinding::new("up", views::SelectPrevCommit, None),
                KeyBinding::new("down", views::SelectNextCommit, None),
                KeyBinding::new("ctrl-n", views::CreateBranch, None),
                KeyBinding::new("ctrl-shift-s", views::StashPush, None),
                KeyBinding::new("ctrl-shift-o", views::StashPop, None),
                KeyBinding::new("ctrl-shift-p", views::ShowCommandPalette, None),
                KeyBinding::new("ctrl-shift-f", views::FetchAll, None),
                KeyBinding::new("ctrl-shift-u", views::PullCurrent, None),
                KeyBinding::new("ctrl-shift-h", views::PushCurrent, None),
                KeyBinding::new("ctrl-shift-t", views::ToggleTheme, None),
            ]);

            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(1440.0), px(900.0)),
                        cx,
                    ))),
                    titlebar: None,
                    window_decorations: Some(WindowDecorations::Client),
                    window_background: WindowBackgroundAppearance::Transparent,
                    app_id: Some("dev.gitforge.GitForge".into()),
                    ..Default::default()
                },
                |_window, cx| {
                    let view = cx.new(|cx| GitForgeApp::new(cx));
                    view.update(cx, |app, cx| app.restore_open_repo_tabs(cx));
                    view.focus_handle(cx).focus(_window);
                    view
                },
            )
            .expect("Failed to open window");
        });
}
