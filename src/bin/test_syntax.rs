use syntect_assets::assets::HighlightingAssets;
use std::path::Path;

fn main() {
    let assets = HighlightingAssets::from_binary();
    let ss = assets.get_syntax_set().unwrap();
    
    for filename in ["config.toml", "script.sh", ".bashrc", "Makefile", "Cargo.lock"] {
        let path = Path::new(filename);
        let name_str = path.file_name().and_then(|x| x.to_str());
        let ext_str = path.extension().and_then(|x| x.to_str());
        
        let syntax = name_str.and_then(|n| ss.find_syntax_by_extension(n))
            .or_else(|| ext_str.and_then(|e| ss.find_syntax_by_extension(e)));
            
        println!("{}: {:?}", filename, syntax.map(|s| &s.name));
    }
}
