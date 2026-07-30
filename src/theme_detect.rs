//! Terminal background light/dark detection for startup theming.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BgKind {
	Light,
	Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectResult {
	pub bg: BgKind,
	/// True when an OSC colour query was sent. Caller must drain leftover
	/// stdin replies before the first editor frame.
	pub osc_attempted: bool,
}

/// Parse `COLORFGBG` (`fg;bg` or `fg;…;bg` ANSI colour indices 0–15).
///
/// Light backgrounds: 7 (white) and 9–15. Dark: 0–6 and 8 (bright black).
/// Requires at least one `;` so a bare number is not treated as background.
pub fn parse_colorfgbg(value: &str) -> Option<BgKind> {
	if !value.contains(';') {
		return None;
	}
	let bg: u8 = value
		.split(';')
		.next_back()?
		.trim()
		.parse()
		.ok()?;
	if bg > 15 {
		return None;
	}
	Some(if is_light_ansi_bg(bg) {
		BgKind::Light
	} else {
		BgKind::Dark
	})
}

fn is_light_ansi_bg(bg: u8) -> bool {
	matches!(bg, 7 | 9..=15)
}

pub struct DetectOptions {
	/// When true, may send OSC 10/11 via terminal-colorsaurus.
	pub allow_osc: bool,
	/// Injected `COLORFGBG` for tests; `None` means absent.
	pub colorfgbg: Option<String>,
	pub osc_timeout: std::time::Duration,
}

impl DetectOptions {
	/// Production options: read `COLORFGBG` from the environment.
	pub fn from_env(allow_osc: bool) -> Self {
		Self {
			allow_osc,
			colorfgbg: std::env::var("COLORFGBG").ok(),
			osc_timeout: std::time::Duration::from_millis(200),
		}
	}
}

pub fn detect_background(opts: DetectOptions) -> DetectResult {
	if let Some(ref raw) = opts.colorfgbg {
		if let Some(bg) = parse_colorfgbg(raw) {
			return DetectResult {
				bg,
				osc_attempted: false,
			};
		}
	}

	if !opts.allow_osc {
		return DetectResult {
			bg: BgKind::Dark,
			osc_attempted: false,
		};
	}

	let mut query = terminal_colorsaurus::QueryOptions::default();
	query.timeout = opts.osc_timeout;
	let mode = terminal_colorsaurus::theme_mode(query);
	let bg = match mode {
		Ok(terminal_colorsaurus::ThemeMode::Light) => BgKind::Light,
		Ok(terminal_colorsaurus::ThemeMode::Dark) | Err(_) => BgKind::Dark,
	};
	DetectResult {
		bg,
		osc_attempted: true,
	}
}

/// Map config theme + detection into (syntect_theme_name, is_light_chrome).
pub fn resolve_themes(config_theme: &str, bg: BgKind) -> (String, bool) {
	let is_light = bg == BgKind::Light;
	if config_theme == "default" {
		let name = if is_light {
			"OneHalfLight".to_string()
		} else {
			"OneHalfDark".to_string()
		};
		(name, is_light)
	} else {
		(config_theme.to_string(), is_light)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn colorfgbg_dark_bg() {
		// Light fg (15), dark bg (0)
		assert_eq!(parse_colorfgbg("15;0"), Some(BgKind::Dark));
	}

	#[test]
	fn colorfgbg_light_bg() {
		assert_eq!(parse_colorfgbg("0;15"), Some(BgKind::Light));
		assert_eq!(parse_colorfgbg("0;7"), Some(BgKind::Light));
	}

	#[test]
	fn colorfgbg_bright_black_is_dark() {
		// Index 8 is bright black — treat as dark.
		assert_eq!(parse_colorfgbg("15;8"), Some(BgKind::Dark));
	}

	#[test]
	fn colorfgbg_rejects_garbage() {
		assert_eq!(parse_colorfgbg(""), None);
		assert_eq!(parse_colorfgbg("nope"), None);
		assert_eq!(parse_colorfgbg("15"), None);
		assert_eq!(parse_colorfgbg("15;"), None);
		assert_eq!(parse_colorfgbg("15;999"), None);
	}

	#[test]
	fn colorfgbg_uses_last_component() {
		// Some terminals append extra fields; last index is background.
		assert_eq!(parse_colorfgbg("0;1;15"), Some(BgKind::Light));
	}

	#[test]
	fn detect_prefers_colorfgbg_and_skips_osc() {
		let r = detect_background(DetectOptions {
			allow_osc: true,
			colorfgbg: Some("0;15".into()),
			osc_timeout: std::time::Duration::from_millis(50),
		});
		assert_eq!(r.bg, BgKind::Light);
		assert!(!r.osc_attempted, "must not probe OSC when COLORFGBG works");
	}

	#[test]
	fn detect_without_osc_permission_falls_back_dark() {
		let r = detect_background(DetectOptions {
			allow_osc: false,
			colorfgbg: None,
			osc_timeout: std::time::Duration::from_millis(50),
		});
		assert_eq!(r.bg, BgKind::Dark);
		assert!(!r.osc_attempted);
	}

	#[test]
	fn detect_invalid_colorfgbg_without_osc_is_dark() {
		let r = detect_background(DetectOptions {
			allow_osc: false,
			colorfgbg: Some("bogus".into()),
			osc_timeout: std::time::Duration::from_millis(50),
		});
		assert_eq!(r.bg, BgKind::Dark);
		assert!(!r.osc_attempted);
	}

	#[test]
	fn default_theme_follows_bg() {
		assert_eq!(
			resolve_themes("default", BgKind::Light),
			("OneHalfLight".into(), true)
		);
		assert_eq!(
			resolve_themes("default", BgKind::Dark),
			("OneHalfDark".into(), false)
		);
	}

	#[test]
	fn explicit_theme_kept_chrome_still_tracks_bg() {
		assert_eq!(
			resolve_themes("DarkNeon", BgKind::Light),
			("DarkNeon".into(), true)
		);
	}
}
