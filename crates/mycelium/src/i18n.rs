//! Строки Forma Mycelium: `locales/<язык>/*.ftl` в корне репозитория. Строки платформы
//! лежат в самом `forma-i18n` и добавляются к ним автоматически.

use std::path::Path;

// Встроенные переводы программы: `pub static LOCALES`.
include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// Загружает переводы программы и пользовательские из папки `extra`.
pub fn init(extra: Option<&Path>) {
    forma_i18n::init_with(&[LOCALES], extra);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn all_languages_have_the_same_keys() {
        let reference = forma_i18n::message_ids(&[LOCALES], forma_i18n::FALLBACK);
        assert!(!reference.is_empty());
        for (code, _) in LOCALES {
            let ids = forma_i18n::message_ids(&[LOCALES], code);
            let missing: Vec<_> = reference.iter().filter(|k| !ids.contains(k)).collect();
            let extra: Vec<_> = ids.iter().filter(|k| !reference.contains(k)).collect();
            assert!(missing.is_empty(), "в языке «{code}» не хватает строк: {missing:?}");
            assert!(extra.is_empty(), "в языке «{code}» лишние строки: {extra:?}");
        }
    }

    #[test]
    fn keys_do_not_repeat_platform_or_each_other() {
        for (code, _) in LOCALES {
            let ids = forma_i18n::message_ids(&[forma_i18n::PLATFORM, LOCALES], code);
            let mut seen = HashSet::new();
            let dups: Vec<_> = ids.iter().filter(|k| !seen.insert(k.as_str())).collect();
            assert!(dups.is_empty(), "в языке «{code}» повторяются ключи: {dups:?}");
        }
    }

    /// Каждый ключ макроса `t!` в крейтах Mycelium переведён (в строках программы или платформы).
    #[test]
    fn every_used_key_exists() {
        let crates = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let mut used = Vec::new();
        for entry in std::fs::read_dir(&crates).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("mycelium")) {
                used.extend(forma_i18n::used_keys(&path));
            }
        }
        assert!(!used.is_empty());
        let ids = forma_i18n::message_ids(&[forma_i18n::PLATFORM, LOCALES], forma_i18n::FALLBACK);
        let missing: Vec<_> = used.iter().filter(|(k, _)| !ids.contains(k)).collect();
        assert!(missing.is_empty(), "ключи используются в коде, но не переведены: {missing:?}");
    }

    #[test]
    fn russian_plurals() {
        init(None);
        let ru = |n: i64| {
            forma_i18n::set_language("ru");
            forma_i18n::t!("status-detached-windows", count = n)
        };
        assert_eq!(ru(1), "1 отдельное окно");
        assert_eq!(ru(3), "3 отдельных окна");
        assert_eq!(ru(5), "5 отдельных окон");
        assert_eq!(ru(21), "21 отдельное окно");
        forma_i18n::set_language("en");
    }
}
