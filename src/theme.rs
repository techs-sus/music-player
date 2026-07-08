use gpui::{App, Global, Rgba, rgb};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{ops::Deref, sync::Arc};

#[derive(Deserialize, Serialize)]
pub struct TintedTheme {
	pub system: String,
	pub name: String,
	pub author: String,
	pub variant: String,
	pub palette: Theme,
}

pub fn serialize_hex_color<S>(value: &Rgba, s: S) -> Result<S::Ok, S::Error>
where
	S: Serializer,
{
	let r = (value.r * 255.0) as u8;
	let g = (value.g * 255.0) as u8;
	let b = (value.b * 255.0) as u8;
	s.serialize_str(&format!("#{r:02X}{g:02X}{b:02X}"))
}

pub fn deserialize_hex_color<'de, D>(d: D) -> Result<Rgba, D::Error>
where
	D: Deserializer<'de>,
{
	let str = String::deserialize(d)?;
	let Some(stripped) = str.strip_prefix("#") else {
		return Err(serde::de::Error::custom("unable to strip prefix"));
	};

	Ok(rgb(
		u32::from_str_radix(stripped, 16).map_err(serde::de::Error::custom)?,
	))
}

#[derive(Deserialize, Serialize)]
pub struct Theme {
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base00: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base01: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base02: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base03: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base04: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base05: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base06: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base07: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base08: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	pub base09: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	#[serde(rename = "base0A")]
	pub base10: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	#[serde(rename = "base0B")]
	pub base11: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	#[serde(rename = "base0C")]
	pub base12: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	#[serde(rename = "base0D")]
	pub base13: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	#[serde(rename = "base0E")]
	pub base14: Rgba,
	#[serde(
		serialize_with = "serialize_hex_color",
		deserialize_with = "deserialize_hex_color"
	)]
	#[serde(rename = "base0F")]
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
