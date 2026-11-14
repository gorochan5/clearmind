fn main() {
    println!("cargo:rerun-if-changed=resources/icon.ico");
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("resources/icon.ico");
        res.compile().expect("Failed to compile Windows resources");
    }
}
