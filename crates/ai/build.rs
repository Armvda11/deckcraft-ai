use std::{io, path::PathBuf};
fn main() {
    if let Err(e) = generate() {
        println!("cargo::error=equation font setup: {e}");
    }
}
fn generate() -> io::Result<()> {
    println!("cargo::rerun-if-env-changed=CRAFT_FONTS_DIR");
    let dir = std::env::var_os("CRAFT_FONTS_DIR").map(PathBuf::from).unwrap_or_default();
    let font = dir.join("fonts/libertinus/LibertinusMath-Regular.otf");
    println!("cargo::rerun-if-changed={}", font.display());
    let data = if font.exists() {
        format!("pub static MATH_FONT: &[u8] = include_bytes!({:?});", font.canonicalize()?.display().to_string())
    } else {
        let native = std::env::var("CARGO_CFG_TARGET_ARCH").is_ok_and(|arch| arch != "wasm32");
        if native {
            return Err(io::Error::new(io::ErrorKind::NotFound, format!("{} is missing; run scripts/fetch-math-font.sh", font.display())));
        }
        "pub static MATH_FONT: &[u8] = &[];".into()
    };
    let out = std::env::var_os("OUT_DIR").map(PathBuf::from).ok_or_else(|| io::Error::other("OUT_DIR missing"))?;
    std::fs::write(out.join("math_font.rs"), data)
}
