import os
import re

def process_file(path):
    with open(path, 'r') as f:
        content = f.read()

    # Special handling for text.rs to inject the helper functions
    if path.endswith("text.rs"):
        # Add theme_bg and theme_fg
        helpers = """
pub fn theme_bg(editor: &Editor) -> Color {
	if !editor.config.syntax_highlight { return Color::Reset; }
	editor.highlighter.theme.settings.background
		.map(syntect_to_crossterm)
		.unwrap_or(Color::Reset)
}

pub fn theme_fg(editor: &Editor) -> Color {
	if !editor.config.syntax_highlight { return Color::Reset; }
	editor.highlighter.theme.settings.foreground
		.map(syntect_to_crossterm)
		.unwrap_or(Color::Reset)
}
"""
        # Inject helpers after subtle_highlight_bg
        content = re.sub(r'(fn subtle_highlight_bg.*?^})', r'\1' + helpers, content, flags=re.MULTILINE | re.DOTALL)

    # In text.rs replace unwrap_or(Color::Reset) for syntax_fg
    if path.endswith("text.rs"):
        content = re.sub(r'unwrap_or\(Color::Reset\)', 'unwrap_or(theme_fg(editor))', content)
        content = re.sub(r'prev_syn_fg = Color::Reset;', 'prev_syn_fg = theme_fg(editor);', content)
        content = re.sub(r'let mut prev_syn_fg = Color::Reset;', 'let mut prev_syn_fg = theme_fg(editor);', content)
        content = re.sub(r'if want_sel \|\| in_search \{ Color::Reset \}', 'if want_sel || in_search { theme_fg(editor) }', content)
        content = re.sub(r'\} else \{\n\t\t\tColor::Reset\n\t\t\};', '} else {\n\t\t\ttheme_bg(editor)\n\t\t};', content)

    # Replace specific SetColor calls everywhere
    content = re.sub(r'w\.queue\(SetBackgroundColor\(Color::Reset\)\)\?;', 'w.queue(SetBackgroundColor(theme_bg(editor)))?;', content)
    content = re.sub(r'w\.queue\(SetForegroundColor\(Color::Reset\)\)\?;', 'w.queue(SetForegroundColor(theme_fg(editor)))?;', content)

    with open(path, 'w') as f:
        f.write(content)

for root, _, files in os.walk("src/render"):
    for file in files:
        if file.endswith(".rs"):
            process_file(os.path.join(root, file))

