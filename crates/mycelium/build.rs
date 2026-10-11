//! Сборочный скрипт: рисует иконку приложения (тот же код, что и в `src/icon.rs`),
//! собирает из неё .ico нескольких размеров и вшивает в .exe вместе со сведениями
//! о продукте. Поэтому проводник и диспетчер задач показывают «Forma Mycelium».

use std::path::PathBuf;

#[path = "src/icon.rs"]
mod icon;

fn main() {
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let ico = out.join("forma-mycelium.ico");
    std::fs::write(&ico, ico_file(&[16, 20, 24, 32, 40, 48, 64, 96, 128, 256])).expect("не удалось записать .ico");
    let rc = out.join("app.rc");
    std::fs::write(&rc, resource_script(&ico)).expect("не удалось записать .rc");
    // Если компилятор ресурсов не найден, просто собираемся без иконки у файла.
    if let Err(err) = embed_resource::compile(&rc, embed_resource::NONE).manifest_optional() {
        println!("cargo:warning=Иконка не вшита в .exe: {err}");
    }
}

/// Сценарий ресурсов: иконка и VERSIONINFO с названием продукта.
fn resource_script(ico: &std::path::Path) -> String {
    let ico_path = ico.display().to_string().replace('\\', "\\\\");
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
    let mut parts: Vec<u16> = version.split(['.', '-']).filter_map(|p| p.parse().ok()).collect();
    parts.resize(4, 0);
    let commas = format!("{},{},{},{}", parts[0], parts[1], parts[2], parts[3]);
    format!(
        r#"1 ICON "{ico_path}"

1 VERSIONINFO
FILEVERSION {commas}
PRODUCTVERSION {commas}
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904B0"
    BEGIN
      VALUE "CompanyName", "FORMA"
      VALUE "FileDescription", "Forma Mycelium"
      VALUE "ProductName", "Forma Mycelium"
      VALUE "FileVersion", "{version}"
      VALUE "ProductVersion", "{version}"
      VALUE "InternalName", "forma-mycelium"
      VALUE "OriginalFilename", "Forma Mycelium.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#
    )
}

/// Файл .ico: заголовок, каталог и картинки в формате 32-битного BMP (с альфа-каналом).
fn ico_file(sizes: &[u32]) -> Vec<u8> {
    let images: Vec<Vec<u8>> = sizes.iter().map(|&s| bmp_entry(s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // зарезервировано
    out.extend_from_slice(&1u16.to_le_bytes()); // тип: иконка
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (&s, img) in sizes.iter().zip(&images) {
        let dim = if s >= 256 { 0 } else { s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // плоскости
        out.extend_from_slice(&32u16.to_le_bytes()); // бит на пиксель
        out.extend_from_slice(&(img.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += img.len() as u32;
    }
    for img in images {
        out.extend_from_slice(&img);
    }
    out
}

/// Одна картинка .ico: BITMAPINFOHEADER, пиксели BGRA снизу вверх и пустая маска.
fn bmp_entry(size: u32) -> Vec<u8> {
    let rgba = icon::rgba(size);
    let mask_row = size.div_ceil(32) * 4;
    let mut out = Vec::new();
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(size as i32).to_le_bytes());
    out.extend_from_slice(&(size as i32 * 2).to_le_bytes()); // высота картинки и маски
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // без сжатия
    out.extend_from_slice(&(size * size * 4 + mask_row * size).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]); // разрешение и палитра не нужны
    for y in (0..size).rev() {
        for x in 0..size {
            let i = ((y * size + x) * 4) as usize;
            out.extend_from_slice(&[rgba[i + 2], rgba[i + 1], rgba[i], rgba[i + 3]]);
        }
    }
    out.extend(std::iter::repeat_n(0u8, (mask_row * size) as usize));
    out
}
