use syntect_assets::assets::HighlightingAssets;
fn main() {
    let assets = HighlightingAssets::from_binary();
    let ss = assets.get_syntax_set().unwrap();
    for syntax in ss.syntaxes() {
        println!("{} - {:?}", syntax.name, syntax.file_extensions);
    }
}
