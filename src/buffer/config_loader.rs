use std::path::Path;
use crate::buffer::Buffer;

/// Deeply inspects the nested path hierarchy natively parsing applicable `.editorconfig` components and strictly applies mapped constraints natively cleanly mutating Buffer defaults.
pub fn load_project_settings(path: &Path, buffer: &mut Buffer) {
	if let Ok(conf) = editorconfig::get_config(path) {
		if let Some(style) = conf.get("indent_style") {
			buffer.expand_tab = Some(style == "space");
		}
		if let Some(size) = conf.get("indent_size") {
			if let Ok(w) = size.parse::<usize>() {
				buffer.tab_width = Some(w);
			}
		}
		if let Some(trim) = conf.get("trim_trailing_whitespace") {
			buffer.trim_on_save = Some(trim == "true");
		}
		if let Some(eol) = conf.get("end_of_line") {
			buffer.newline_style = Some(eol.to_string());
		}
	}
}
