mod theme;
use anyhow::Context as _;
use directories::ProjectDirs;
use theme::*;

use gpui::{App, Context, Entity, Length, MouseButton, Window, WindowOptions, div, prelude::*};
use std::sync::Arc;

enum Page {
	Playlists,
	FullscreenPlayer,
}

struct Sidebar {
	page: Entity<Page>,
}

impl Render for Sidebar {
	fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let theme = cx.theme();

		div()
			.bg(theme.base01)
			.w(Length::Definite(gpui::DefiniteLength::Fraction(0.1)))
			.h_full()
	}
}

struct Viewport {
	page: Entity<Page>,
}

impl Render for Viewport {
	fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
		let sidebar = cx.new(|_| Sidebar {
			page: self.page.clone(),
		});
		let theme = cx.theme();

		div()
			.flex()
			.flex_row()
			.gap_3()
			.bg(theme.base00)
			.size_full()
			.justify_start()
			.items_start()
			.shadow_lg()
			.child(sidebar)
			.child(
				div()
					.flex()
					.justify_center()
					.items_center()
					.size_auto()
					.text_xl()
					.m_4()
					.p_2()
					.text_color(theme.base05)
					.on_any_mouse_down(|event, _window, app| {
						if matches!(event.button, MouseButton::Left) {
							app.quit();
						};
					})
					.bg(theme.base02)
					.border_color(theme.base05)
					.rounded_md()
					.child("Quit"),
			)
	}
}

fn get_theme(dirs: Option<ProjectDirs>) -> anyhow::Result<Theme> {
	let theme_yaml_path = dirs
		.context("no ProjectDirs found")?
		.config_dir()
		.join("theme.yaml");

	Ok(
		serde_yaml_ng::from_reader::<_, TintedTheme>(
			std::fs::File::open(theme_yaml_path).context("failed opening theme.yaml for reading")?,
		)
		.context("failed decoding theme.yaml")?
		.palette,
	)
}

fn main() {
	let dirs = ProjectDirs::from("com", "techs-sus", "Music Player");

	if let Some(ref dirs) = dirs {
		// create config directory so that users can find it and insert a theme.yaml into it
		let _ = std::fs::create_dir(dirs.config_dir());
	};

	let theme = get_theme(dirs).unwrap_or_else(|err| {
		eprintln!("failed loading theme from config path: {err}");

		DEFAULT_BASE16_THEME
	});

	gpui_platform::application().run(|cx: &mut App| {
		cx.set_global::<GlobalTheme>(GlobalTheme(Arc::new(theme)));

		cx.open_window(
			WindowOptions {
				focus: true,
				app_id: Some("com.github.techs-sus.music-player".to_string()),
				..Default::default()
			},
			|_, cx| {
				let page = cx.new(|_| Page::Playlists);

				cx.new(|_| Viewport { page })
			},
		)
		.unwrap();
	});
}
