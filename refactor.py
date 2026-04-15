import re

with open("src/render/chrome.rs", "r") as f:
    code = f.read()

# 1. Option<PromptLayout> -> Option<Window>
code = code.replace("pub fn build_prompt(editor: &Editor, width: u16) -> Option<PromptLayout> {", "pub fn build_prompt(editor: &Editor, vp: &Viewport) -> Option<Window> {\n\tlet width = vp.width;")

# 2. PromptBlock -> UiFragment
code = re.sub(r'PromptBlock \{\s*bg:\s*([^,]+),\s*fg:\s*([^,]+),\s*text:\s*([^,]+),\s*\}', r'UiFragment { bg: \1, fg: \2, text: \3, is_flex: false, }', code)

# 3. PromptLayout -> Window
def repl_layout(m):
    rows = m.group(1).strip()
    middle = m.group(2)
    # convert cursor_offset
    middle = re.sub(r'cursor_offset:\s*0\s*,', r'cursor_bounds: None,', middle)
    middle = re.sub(r'cursor_offset,', r'cursor_bounds: Some((cursor_offset, 0)),', middle)
    # convert blocks
    middle = re.sub(r'blocks,?', r'fragments: blocks,', middle)
    
    return f"Some(Window {{\n\t\t\t\trect: Rect {{ x: 0, y: 0, width: w as u16, height: {rows} }},\n\t\t\t\tgravity: Gravity::BottomLeft,\n\t\t\t\tz_index: 10,\n{middle}\n\t\t\t}})"

code = re.sub(r'Some\(PromptLayout \{\s*rows:?([^,]*),\s*([\s\S]*?)\}\)', repl_layout, code)

# 4. padding replace
# search for the pad_len block
pad_block = r'''				if pad_len > 0 \{
					blocks\.push\(UiFragment \{
						bg: toolbar_bg_color\(editor\),
						fg: Color::DarkGrey,
						text: " "\.repeat\(pad_len\),
						is_flex: false,
					\}\);
				\}'''
flex_repl = '''				blocks.push(UiFragment {
					bg: editor.theme.toolbar_bg,
					fg: editor.theme.toolbar_bg,
					text: String::new(),
					is_flex: true,
				});'''
code = re.sub(pad_block, flex_repl, code)

# 5. Remove PromptBlock/PromptLayout structs entirely
code = re.sub(r'pub struct PromptBlock \{[\s\S]*?\}\n\npub struct PromptLayout \{[\s\S]*?\}\n\n', '', code)

# 6. Change `let blocks = vec!` inside Prompt matching
code = code.replace("let blocks = vec![UiFragment", "let mut blocks = vec![UiFragment")

with open("src/render/chrome.rs", "w") as f:
    f.write(code)
