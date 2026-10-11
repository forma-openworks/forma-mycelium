//! Печатает внешние библиотеки проекта по группам: `cargo run -p mycelium-project --example libraries -- <папка мода> [model.json]`.

use mycelium_project::{GradleModel, Libraries, detect};

fn main() {
    let mut args = std::env::args().skip(1);
    let root = std::path::PathBuf::from(args.next().expect("папка мода"));
    let model: GradleModel = match args.next() {
        Some(path) => serde_json::from_str(&std::fs::read_to_string(path).expect("модель читается")).expect("модель разбирается"),
        None => GradleModel::load(&root).expect("в проекте нет .mycelium/cache/model.json"),
    };
    let info = detect::detect(&root);
    let started = std::time::Instant::now();
    let libs = Libraries::build(&model, &info);
    println!("jar: {}, за {:?}", libs.libs.len(), started.elapsed());
    for g in &libs.groups {
        println!("== {:?} {:?}: частей {}, отдельных {}", g.kind, g.version, g.parts.len(), g.members.len());
        let started = std::time::Instant::now();
        let top = libs.children(&g.parts, "");
        println!("   корень ({:?}): {:?}", started.elapsed(), top.iter().map(|n| n.name.as_str()).collect::<Vec<_>>());
        for &m in g.members.iter().take(6) {
            let l = &libs.libs[m];
            println!("   · {} {} исходники={}", l.name, l.version.as_deref().unwrap_or(""), l.sources.is_some());
        }
    }
    if let Some((lib, entry)) = libs.find_class("net.minecraft.world.item.Item") {
        println!("Item → {:?}", libs.source_for(lib, &entry, Some("Item.java")).map(|(_, p)| p));
    }
}
