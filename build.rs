use winresource::WindowsResource;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = WindowsResource::new();

        // Version aus Cargo.toml wird automatisch übernommen
        res.set("ProductName", "WinOMove");
        res.set("FileDescription", "Move windows between monitors with Win+Shift+Arrow keys");
        res.set("LegalCopyright", "MIT License");

        // Icon einbetten
        res.set_icon("icon.ico");

        res.compile().expect("Failed to compile Windows resources");
    }
}
