/// Utility functions for the editor.
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthChar;

/// `path` made absolute against the current directory, without touching
/// symlinks. This is the form stored in `Buffer::file_path` and the recent
/// list: a bare `notes.md` means nothing once it leaves the directory it was
/// typed in.
pub fn absolute_path(path: &Path) -> PathBuf {
	std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}

/// The real identity of the file at `path`: fully canonical when it exists,
/// so relative, `./x`, and symlinked spellings all compare equal. A file that
/// doesn't exist yet (`dan new.md`) gets its parent canonicalized instead.
/// Use this to compare files or key per-file state, never for display.
pub fn file_identity(path: &Path) -> PathBuf {
	if let Ok(p) = std::fs::canonicalize(path) {
		return p;
	}
	let abs = absolute_path(path);
	match (abs.parent(), abs.file_name()) {
		(Some(parent), Some(name)) => std::fs::canonicalize(parent)
			.map(|p| p.join(name))
			.unwrap_or(abs),
		_ => abs,
	}
}

/// True if `a` and `b` name the same file (see `file_identity`).
pub fn same_file(a: &Path, b: &Path) -> bool {
	a == b || file_identity(a) == file_identity(b)
}

/// Display width of a character in terminal columns.
///
/// Uses the `unicode-width` crate for correct handling of:
/// - CJK Unified Ideographs (all extensions)
/// - Fullwidth / Halfwidth Forms
/// - Hangul Syllables
/// - Emoji with `Emoji_Presentation`
/// - Zero-width joiners, combining marks (width 0)
/// - Control characters (width 0)
pub fn char_width(ch: char, tab_width: usize) -> usize {
	match ch {
		'\t' => tab_width,
		'\n' | '\r' => 0,
		c => UnicodeWidthChar::width(c).unwrap_or(0),
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn ascii_width() {
		assert_eq!(char_width('a', 4), 1);
		assert_eq!(char_width('Z', 4), 1);
		assert_eq!(char_width('0', 4), 1);
		assert_eq!(char_width(' ', 4), 1);
		assert_eq!(char_width('!', 4), 1);
	}

	#[test]
	fn tab_uses_configured_width() {
		assert_eq!(char_width('\t', 4), 4);
		assert_eq!(char_width('\t', 8), 8);
		assert_eq!(char_width('\t', 2), 2);
	}

	#[test]
	fn newline_cr_zero() {
		assert_eq!(char_width('\n', 4), 0);
		assert_eq!(char_width('\r', 4), 0);
	}

	#[test]
	fn cjk_ideographs_are_wide() {
		assert_eq!(char_width('中', 4), 2);
		assert_eq!(char_width('文', 4), 2);
		assert_eq!(char_width('字', 4), 2);
	}

	#[test]
	fn hangul_is_wide() {
		assert_eq!(char_width('한', 4), 2);
		assert_eq!(char_width('글', 4), 2);
	}

	#[test]
	fn fullwidth_forms_are_wide() {
		// Fullwidth Latin 'Ａ' (U+FF21)
		assert_eq!(char_width('Ａ', 4), 2);
	}

	#[test]
	fn combining_marks_zero_width() {
		// U+0301 COMBINING ACUTE ACCENT
		assert_eq!(char_width('\u{0301}', 4), 0);
	}

	#[test]
	fn control_chars_zero_width() {
		// Null, BEL, ESC
		assert_eq!(char_width('\0', 4), 0);
		assert_eq!(char_width('\x07', 4), 0);
		assert_eq!(char_width('\x1B', 4), 0);
	}

	#[test]
	fn emoji_presentation_is_wide() {
		// README comparison table uses these; terminals draw them 2 columns wide.
		assert_eq!(char_width('✅', 4), 2);
		assert_eq!(char_width('❌', 4), 2);
		assert_eq!(char_width('🚀', 4), 2);
	}

	#[test]
	fn absolute_path_anchors_relative_paths_to_cwd() {
		let cwd = std::env::current_dir().unwrap();
		assert_eq!(absolute_path(Path::new("notes.md")), cwd.join("notes.md"));
	}

	#[cfg(unix)]
	#[test]
	fn same_file_sees_through_spelling_and_symlinks() {
		let dir = std::env::temp_dir().join(format!("dan_utils_ident_{}", std::process::id()));
		let real = dir.join("real");
		std::fs::create_dir_all(&real).unwrap();
		let link = dir.join("link");
		let _ = std::fs::remove_file(&link);
		std::os::unix::fs::symlink(&real, &link).unwrap();
		let f = real.join("f.txt");
		std::fs::write(&f, "x").unwrap();

		assert!(same_file(&f, &real.join(".").join("f.txt")));
		assert!(same_file(&f, &link.join("f.txt")));
		// Not-yet-existing files still compare through a symlinked parent.
		assert!(same_file(&real.join("new.txt"), &link.join("new.txt")));
		assert!(!same_file(&f, &real.join("other.txt")));

		std::fs::remove_dir_all(&dir).ok();
	}
}
