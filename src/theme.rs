use gpui::{App, Global, Rgba};
use std::{ops::Deref, sync::Arc};

pub struct Theme {
	pub base00: Rgba,
	pub base01: Rgba,
	pub base02: Rgba,
	pub base03: Rgba,
	pub base04: Rgba,
	pub base05: Rgba,
	pub base06: Rgba,
	pub base07: Rgba,
	pub base08: Rgba,
	pub base09: Rgba,
	pub base10: Rgba,
	pub base11: Rgba,
	pub base12: Rgba,
	pub base13: Rgba,
	pub base14: Rgba,
	pub base15: Rgba,
}

pub struct GlobalTheme(pub Arc<Theme>);

impl Global for GlobalTheme {}

impl Deref for GlobalTheme {
	type Target = Arc<Theme>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub const fn hex(c: u32) -> Rgba {
	let [_, r, g, b] = c.to_be_bytes();
	Rgba {
		r: r as f32 / 255.0,
		g: g as f32 / 255.0,
		b: b as f32 / 255.0,
		a: 1.0,
	}
}

pub const DEFAULT_BASE16_THEME: Theme = Theme {
	base00: hex(0x282828),
	base01: hex(0x3c3836),
	base02: hex(0x504945),
	base03: hex(0x665c54),
	base04: hex(0x928374),
	base05: hex(0xebdbb2),
	base06: hex(0xfbf1c7),
	base07: hex(0xf9f5d7),
	base08: hex(0xcc241d),
	base09: hex(0xd65d0e),
	base10: hex(0xd79921),
	base11: hex(0x98971a),
	base12: hex(0x689d6a),
	base13: hex(0x458588),
	base14: hex(0xb16286),
	base15: hex(0x9d0006),
};

pub trait ActiveTheme {
	fn theme(&self) -> &Arc<Theme>;
}

impl ActiveTheme for App {
	fn theme(&self) -> &Arc<Theme> {
		&self.global::<GlobalTheme>().0
	}
}
