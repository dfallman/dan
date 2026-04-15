use syntect::highlighting::{Theme, ThemeSettings};
use syntect_assets::assets::HighlightingAssets;
fn main() {
    let assets = HighlightingAssets::from_binary();
    let theme = assets.get_theme("OneHalfDark").clone();
    println!("Theme scopes count: {}", theme.scopes.len());
    for item in theme.scopes.iter().take(5) {
        // can we print the scope?
        // item.scope is of type ScopeSelectors
        // Wait, ScopeSelectors doesn't derive Display, maybe Debug?
        println!("{:?}", item.scope);
    }
}
