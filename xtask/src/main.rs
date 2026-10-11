//! Служебные задачи сборки. Запуск: `cargo xtask <задача>`.
//!
//! - `dist` — релизная сборка и готовый `dist/Forma Mycelium.exe`
//!   (Cargo не разрешает пробел в имени бинарника, поэтому файл переименовывается здесь).

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const BIN: &str = "forma-mycelium";
const PRODUCT_FILE: &str = "Forma Mycelium";

fn main() -> ExitCode {
    let task = std::env::args().nth(1);
    let result = match task.as_deref() {
        Some("dist") => dist(),
        _ => {
            eprintln!("Использование: cargo xtask dist");
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("Ошибка: {err}");
            ExitCode::FAILURE
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("корень репозитория").to_path_buf()
}

fn dist() -> Result<(), String> {
    let root = root();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(&root)
        .args(["build", "--release", "--package", "mycelium", "--bin", BIN])
        .status()
        .map_err(|e| format!("не удалось запустить cargo: {e}"))?;
    if !status.success() {
        return Err("сборка не удалась".into());
    }
    let ext = std::env::consts::EXE_SUFFIX;
    let built = root.join("target").join("release").join(format!("{BIN}{ext}"));
    let dist = root.join("dist");
    std::fs::create_dir_all(&dist).map_err(|e| format!("не удалось создать {}: {e}", dist.display()))?;
    let target = dist.join(format!("{PRODUCT_FILE}{ext}"));
    std::fs::copy(&built, &target).map_err(|e| format!("не удалось скопировать {} → {}: {e}", built.display(), target.display()))?;
    println!("Готово: {}", target.display());
    Ok(())
}
