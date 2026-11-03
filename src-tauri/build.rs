fn main() {
    // Rebuild native icon resources when generated icons change.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
