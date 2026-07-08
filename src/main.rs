mod theme;
use theme::*;

use gpui::{
	App, Context, Entity, Length, MouseButton, Window, WindowOptions, div, prelude::*, rgb,
};
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

fn main() {
	gpui_platform::application().run(|cx: &mut App| {
		cx.set_global::<GlobalTheme>(GlobalTheme(Arc::new(DEFAULT_BASE16_THEME)));

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
