//! Иконка Forma Mycelium для активности в Discord (подробный рисунок — там она крупная).
//! Discord берёт картинки по ссылке из публичного репозитория, поэтому они лежат в
//! `assets/discord/`. Значки загрузчиков там же, но это готовые официальные картинки
//! NeoForge и Fabric (откуда они — в `assets/discord/README.md`), их здесь не рисуем.

use std::path::Path;

#[path = "../../crates/mycelium/src/icon.rs"]
#[allow(dead_code)]
mod icon;

fn write_png(path: &Path, size: u32, rgba: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), size, size);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(rgba).map_err(|e| e.to_string())?;
    w.finish().map_err(|e| e.to_string())
}

pub fn render(out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    write_png(&out.join("mycelium.png"), 512, &icon::rgba(icon::Logo::Detailed, 512))?;
    println!("Готово: {}", out.display());
    Ok(())
}
