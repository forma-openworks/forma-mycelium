//! Моды запуска для интерфейса: названия, логотипы и правила происхождения открытого
//! проекта. Обновляется, когда Hyphae разобрал моды; читают консоль, окно исключений
//! и вкладка «Моды запуска» — так плашка мода везде одинаковая: «Create (create)» с логотипом.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};

use eframe::egui;
use mycelium_hyphae::RunMods;
use mycelium_project::{Origin, OriginRules};

/// Сторона логотипа в пикселях после уменьшения.
pub const LOGO_PX: u32 = 64;

#[derive(Default)]
struct State {
    run: Option<Arc<RunMods>>,
    logos: HashMap<String, egui::TextureHandle>,
    rules: OriginRules,
    /// Папки ресурсов своего мода (абсолютные) — по ним карточка проблемы находит файлы.
    resource_dirs: Vec<PathBuf>,
}

fn state() -> &'static RwLock<State> {
    static STATE: OnceLock<RwLock<State>> = OnceLock::new();
    STATE.get_or_init(RwLock::default)
}

/// Новые моды запуска и их логотипы (`None` — проект закрыт).
pub fn set(run: Option<Arc<RunMods>>, logos: HashMap<String, egui::TextureHandle>) {
    let mut s = state().write().unwrap_or_else(|e| e.into_inner());
    s.run = run;
    s.logos = logos;
}

/// Правила происхождения открытого проекта (для разметки сообщений).
pub fn set_rules(rules: &OriginRules) {
    state().write().unwrap_or_else(|e| e.into_inner()).rules = rules.clone();
}

/// Моды запуска, если они уже разобраны.
pub fn run() -> Option<Arc<RunMods>> {
    state().read().unwrap_or_else(|e| e.into_inner()).run.clone()
}

/// Папки ресурсов открытого проекта (пусто — проект закрыт).
pub fn set_resource_dirs(dirs: Vec<PathBuf>) {
    state().write().unwrap_or_else(|e| e.into_inner()).resource_dirs = dirs;
}

pub fn resource_dirs() -> Vec<PathBuf> {
    state().read().unwrap_or_else(|e| e.into_inner()).resource_dirs.clone()
}

pub fn rules() -> OriginRules {
    state().read().unwrap_or_else(|e| e.into_inner()).rules.clone()
}

/// Логотип мода, если он у мода есть.
pub fn logo(id: &str) -> Option<egui::TextureHandle> {
    state().read().unwrap_or_else(|e| e.into_inner()).logos.get(id).cloned()
}

/// id мода, к которому относится происхождение (свой мод — тоже мод).
pub fn mod_id(origin: &Origin) -> Option<String> {
    match origin {
        Origin::OtherMod(Some(id)) => Some(id.clone()),
        Origin::OwnMod => {
            let s = state().read().unwrap_or_else(|e| e.into_inner());
            s.run.as_ref().and_then(|r| r.mods.iter().find(|m| m.kind == mycelium_hyphae::ModKind::Own).map(|m| m.id.clone())).or_else(|| s.rules.mod_ids.first().cloned())
        }
        _ => None,
    }
}

/// Название мода для людей, если оно не совпадает с id.
pub fn mod_name(id: &str) -> Option<String> {
    let s = state().read().unwrap_or_else(|e| e.into_inner());
    s.run.as_ref().and_then(|r| r.get(id)).filter(|m| m.name != m.id).map(|m| m.name.clone())
}

/// Подпись мода: «Название (id)». Мода нет среди разобранных — просто id.
pub fn mod_label(id: &str) -> String {
    let s = state().read().unwrap_or_else(|e| e.into_inner());
    match s.run.as_ref().and_then(|r| r.get(id)) {
        Some(m) if m.name != m.id => format!("{} ({})", m.name, m.id),
        _ => id.to_owned(),
    }
}

/// Уменьшенный логотип: картинка вписывается в квадрат [`LOGO_PX`] с прозрачными полями.
/// Вызывается в фоновом потоке — декодирование большой PNG занимает время.
pub fn decode_logo(bytes: &[u8]) -> Option<egui::ColorImage> {
    let img = image::load_from_memory(bytes).ok()?;
    let small = img.resize(LOGO_PX, LOGO_PX, image::imageops::FilterType::Triangle).to_rgba8();
    let (w, h) = (small.width(), small.height());
    let mut square = image::RgbaImage::new(LOGO_PX, LOGO_PX);
    image::imageops::overlay(&mut square, &small, ((LOGO_PX - w) / 2) as i64, ((LOGO_PX - h) / 2) as i64);
    Some(egui::ColorImage::from_rgba_unmultiplied([LOGO_PX as usize, LOGO_PX as usize], square.as_raw()))
}
