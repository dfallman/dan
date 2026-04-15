with open("src/editor/mod.rs", "r") as f:
    text = f.read()

# Replace struct field
text = text.replace("pub save_as_cursor: usize,", "pub prompt_cursor: usize,\n\tpub prompt_view_start: usize,")

# Replace initialization
text = text.replace("save_as_cursor: 0,", "prompt_cursor: 0,\n\t\t\tprompt_view_start: 0,")

# Replace usages
text = text.replace("save_as_cursor", "prompt_cursor")

with open("src/editor/mod.rs", "w") as f:
    f.write(text)
