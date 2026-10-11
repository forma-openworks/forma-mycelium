//! Картинки для активности в Discord: иконка Forma Mycelium (тот же код, что у .exe)
//! и круглые значки загрузчиков в цветах происхождения из IDE. Discord берёт их по ссылке
//! из публичного репозитория, поэтому они лежат в `assets/discord/`.

use std::path::Path;

#[path = "../../crates/mycelium/src/icon.rs"]
#[allow(dead_code)]
mod icon;

type P = (f32, f32);

/// Значок: цвет круга, цвет знака и сам знак.
struct Badge {
    name: &'static str,
    bg: [u8; 3],
    fg: [u8; 3],
    glyph: fn(P) -> bool,
}

/// Наковальня — знак NeoForge и Forge.
fn anvil(p: P) -> bool {
    const POLY: [P; 14] = [
        (0.15, 0.34),
        (0.82, 0.34),
        (0.82, 0.43),
        (0.66, 0.49),
        (0.60, 0.55),
        (0.60, 0.61),
        (0.71, 0.63),
        (0.73, 0.71),
        (0.27, 0.71),
        (0.29, 0.63),
        (0.40, 0.61),
        (0.40, 0.55),
        (0.33, 0.47),
        (0.15, 0.40),
    ];
    in_polygon(p, &POLY)
}

/// Лоскут с прострочкой — знак Fabric.
fn patch(p: P) -> bool {
    let (x, y) = (p.0 - 0.5, p.1 - 0.5);
    let d = x.abs() + y.abs();
    // Ромб с рамкой и пунктиром стежков внутри.
    let border = (0.27..0.33).contains(&d);
    // Пунктир отмеряем вдоль своей стороны ромба: на сторонах, где x+y постоянно, — по x−y, и наоборот.
    let along = if x * y >= 0.0 { x - y } else { x + y };
    let stitch = (0.18..0.215).contains(&d) && ((along * 22.0).floor() as i32).rem_euclid(2) == 0;
    border || stitch || d < 0.06
}

/// Лоскутное одеяло 3×3 — знак Quilt.
fn quilt(p: P) -> bool {
    let (x, y) = ((p.0 - 0.23) / 0.18, (p.1 - 0.23) / 0.18);
    if !(0.0..3.0).contains(&x) || !(0.0..3.0).contains(&y) {
        return false;
    }
    let (fx, fy) = (x.fract(), y.fract());
    let cell = (x as i32 + y as i32) % 2 == 0;
    let inset = if cell { 0.08 } else { 0.22 };
    (inset..1.0 - inset).contains(&fx) && (inset..1.0 - inset).contains(&fy)
}

fn in_polygon(p: P, poly: &[P]) -> bool {
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[j]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Круглый значок со сглаживанием (4×4 выборки на пиксель).
fn badge(size: u32, b: &Badge) -> Vec<u8> {
    const SS: u32 = 4;
    let mut out = vec![0u8; (size * size * 4) as usize];
    for py in 0..size {
        for px in 0..size {
            let (mut disc, mut mark) = (0.0f32, 0.0f32);
            for sy in 0..SS {
                for sx in 0..SS {
                    let p = ((px as f32 + (sx as f32 + 0.5) / SS as f32) / size as f32, (py as f32 + (sy as f32 + 0.5) / SS as f32) / size as f32);
                    let r = ((p.0 - 0.5).powi(2) + (p.1 - 0.5).powi(2)).sqrt();
                    if r <= 0.5 {
                        disc += 1.0;
                        if (b.glyph)(p) {
                            mark += 1.0;
                        }
                    }
                }
            }
            let n = (SS * SS) as f32;
            let (disc, mark) = (disc / n, mark / n);
            let k = if disc > 0.0 { mark / disc } else { 0.0 };
            let i = ((py * size + px) * 4) as usize;
            for c in 0..3 {
                out[i + c] = (b.bg[c] as f32 * (1.0 - k) + b.fg[c] as f32 * k).round() as u8;
            }
            out[i + 3] = (disc * 255.0).round() as u8;
        }
    }
    out
}

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
    write_png(&out.join("mycelium.png"), 512, &icon::rgba(512))?;
    let badges = [
        Badge { name: "neoforge", bg: [240, 138, 36], fg: [255, 255, 255], glyph: anvil },
        Badge { name: "forge", bg: [64, 110, 180], fg: [255, 255, 255], glyph: anvil },
        Badge { name: "fabric", bg: [219, 198, 160], fg: [74, 58, 40], glyph: patch },
        Badge { name: "quilt", bg: [151, 92, 230], fg: [255, 255, 255], glyph: quilt },
    ];
    for b in &badges {
        write_png(&out.join(format!("{}.png", b.name)), 256, &badge(256, b))?;
    }
    println!("Готово: {}", out.display());
    Ok(())
}
