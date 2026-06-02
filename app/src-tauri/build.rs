fn main() {
    // scraper.js is embedded via include_str! — force rebuild when it changes.
    println!("cargo:rerun-if-changed=scraper.js");
    tauri_build::build()
}
