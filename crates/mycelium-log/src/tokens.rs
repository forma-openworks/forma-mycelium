//! Куски текста записи, у которых есть смысл: классы, id ресурсов, файлы и то, что
//! разобрала классификация (цель mixin, его конфигурация, мод). Интерфейс красит их
//! цветом владельца, показывает о них подсказки и открывает по клику.

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Field, Role};

/// Кусок текста с ролью. `range` — байты в тексте.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub range: Range<usize>,
    pub role: Role,
}

/// id ресурса: `пространство:путь`. Путь — до пробела, кавычки или скобки: в неправильном
/// id бывают и заглавные буквы, и пробелы не бывают.
static RESOURCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b([a-z][a-z0-9_.-]*):([^\s'\x22,;:()\[\]{}<>]+)").expect("регулярное выражение id"));
/// Файл: путь с расширением ресурса, кода или архива.
static FILE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\w./\\-]+\.(?:json|png|obj|mtl|toml|java|kt|ogg|class|jar|mcmeta|nbt|cfg|properties)\b").expect("регулярное выражение файла"));
/// Полное имя класса: `net.minecraft.world.item.Item`, `com.example.Foo$Bar`.
static CLASS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:[a-z_][a-z0-9_]*\.){2,}[A-Z][A-Za-z0-9_$]*").expect("регулярное выражение класса"));
/// Имя класса, как в байткоде: `net/minecraft/client/Minecraft`.
static CLASS_PATH: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b(?:[a-z_][a-z0-9_]*/){2,}[A-Z][A-Za-z0-9_$]*").expect("регулярное выражение класса через /"));

/// Символы, из которых состоят имена: id, классы, пути.
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || "_.-$/".contains(c)
}

/// Слева от `start` — не продолжение имени.
fn starts_word(text: &str, start: usize) -> bool {
    !text[..start].chars().next_back().is_some_and(is_name_char)
}

/// Справа от `end` — не продолжение имени. Точка в конце предложения именем не считается.
fn ends_word(text: &str, end: usize) -> bool {
    let mut rest = text[end..].chars();
    match rest.next() {
        None => true,
        Some('.') => !rest.next().is_some_and(is_name_char),
        Some(c) => !is_name_char(c),
    }
}

/// Куски текста по порядку. Сначала — разобранные поля записи (длинные раньше: id мода
/// бывает началом имени его конфигурации), потом — всё, что узнаётся по виду.
pub fn tokens(text: &str, fields: &[Field]) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::new();
    let push = |out: &mut Vec<Token>, range: Range<usize>, role: Role| {
        if !out.iter().any(|t| range.start < t.range.end && t.range.start < range.end) {
            out.push(Token { range, role });
        }
    };
    let mut fields: Vec<&Field> = fields.iter().filter(|f| !f.value.is_empty()).collect();
    fields.sort_by_key(|f| std::cmp::Reverse(f.value.len()));
    for f in fields {
        for (start, _) in text.match_indices(f.value.as_str()) {
            let end = start + f.value.len();
            if starts_word(text, start) && ends_word(text, end) {
                push(&mut out, start..end, f.role);
            }
        }
    }
    for m in RESOURCE.find_iter(text) {
        let ns = m.as_str().split(':').next().unwrap_or("");
        // `https://…` — адрес, `x.mixins.json:client.Foo` — mixin, `Foo.java:12` — место в файле.
        if m.as_str().contains("://") || ns.ends_with(".json") || !starts_word(text, m.start()) {
            continue;
        }
        push(&mut out, m.range(), Role::Resource);
    }
    // Файлы раньше классов через `/`: `textures/item/Ruby.png` — файл, а не класс `Ruby`.
    for (re, role) in [(&*FILE, Role::File), (&*CLASS, Role::Class), (&*CLASS_PATH, Role::Class)] {
        for m in re.find_iter(text) {
            if starts_word(text, m.start()) {
                push(&mut out, m.range(), role);
            }
        }
    }
    out.sort_by_key(|t| t.range.start);
    out
}

/// Имя класса через точки: в байткоде и в некоторых сообщениях оно через `/`.
pub fn class_name(value: &str) -> String {
    value.replace('/', ".")
}

/// Короткое имя класса: всё после пакета (`KeyBindsList$KeyEntry`).
pub fn simple_name(value: &str) -> &str {
    value.rsplit(['.', '/']).next().unwrap_or(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles(text: &str, fields: &[Field]) -> Vec<(&'static str, String)> {
        tokens(text, fields).into_iter().map(|t| (t.role.key(), text[t.range].to_owned())).collect()
    }

    fn field(role: Role, value: &str) -> Field {
        Field { role, value: value.into() }
    }

    #[test]
    fn mixin_fields_win_over_lookalikes() {
        let text = "@Mixin target net.minecraft.client.gui.screens.controls.KeyBindsList$KeyEntry was not found railways-common.mixins.json:client.MixinKeyBindsListKeyEntry from mod railways";
        let fields = [
            field(Role::MixinTarget, "net.minecraft.client.gui.screens.controls.KeyBindsList$KeyEntry"),
            field(Role::MixinConfig, "railways-common.mixins.json"),
            field(Role::Mixin, "client.MixinKeyBindsListKeyEntry"),
            field(Role::Mod, "railways"),
        ];
        assert_eq!(
            roles(text, &fields),
            vec![
                ("target", "net.minecraft.client.gui.screens.controls.KeyBindsList$KeyEntry".into()),
                ("config", "railways-common.mixins.json".into()),
                ("mixin", "client.MixinKeyBindsListKeyEntry".into()),
                ("mod", "railways".into()),
            ],
            "railways внутри имени конфигурации — не мод"
        );
    }

    #[test]
    fn recognisable_by_shape() {
        assert_eq!(
            roles("Missing textures in model testmod:item/ruby#inventory, see https://x.org/a", &[]),
            vec![("resource", "testmod:item/ruby#inventory".into())],
            "адрес в интернете — не id"
        );
        assert_eq!(
            roles("Error loading class: net/minecraft/client/gui/Foo$Bar (java.lang.ClassNotFoundException: net.minecraft.client.gui.Foo$Bar)", &[]),
            vec![
                ("class", "net/minecraft/client/gui/Foo$Bar".into()),
                ("class", "java.lang.ClassNotFoundException".into()),
                ("class", "net.minecraft.client.gui.Foo$Bar".into()),
            ]
        );
        assert_eq!(roles("at Foo.java:12 in assets/testmod/textures/item/Ruby.png", &[]), vec![("file", "Foo.java".into()), ("file", "assets/testmod/textures/item/Ruby.png".into())]);
        assert_eq!(roles("Cannot invoke \"net.minecraft.world.item.Item.getDescriptionId()\"", &[]), vec![("class", "net.minecraft.world.item.Item".into())]);
    }

    #[test]
    fn ids_after_ids_are_separate() {
        assert_eq!(
            roles("minecraft:textures/atlas/blocks.png:testmod:item/ruby", &[]),
            vec![("resource", "minecraft:textures/atlas/blocks.png".into()), ("resource", "testmod:item/ruby".into())]
        );
    }

    #[test]
    fn fields_need_whole_words() {
        let text = "mod railways-extra failed; blame railways.";
        assert_eq!(roles(text, &[field(Role::Mod, "railways")]), vec![("mod", "railways".into())], "точка в конце предложения — не часть имени");
        assert_eq!(tokens(text, &[field(Role::Mod, "railways")])[0].range.start, text.rfind("railways").unwrap());
    }

    #[test]
    fn names() {
        assert_eq!(simple_name("net.minecraft.client.gui.screens.controls.KeyBindsList$KeyEntry"), "KeyBindsList$KeyEntry");
        assert_eq!(simple_name("client.MixinFoo"), "MixinFoo");
        assert_eq!(class_name("net/minecraft/Foo$Bar"), "net.minecraft.Foo$Bar");
    }
}
