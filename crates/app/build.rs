fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winres::WindowsResource::new();
        // If the icon exists, embed it. In dev, it might not exist yet, so we ignore errors if it doesn't.
        let icon_path = "../../assets/app.ico";
        if std::path::Path::new(icon_path).exists() {
            res.set_icon(icon_path);
        }
        res.compile().unwrap();
    }
}
