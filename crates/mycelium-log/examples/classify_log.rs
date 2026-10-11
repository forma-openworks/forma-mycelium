//! Разбирает файл лога и печатает найденные проблемы — для проверки классификации
//! на настоящих логах: `cargo run -p mycelium-log --example classify_log -- latest.log com.example.mymod mymod`.

use mycelium_log::{Parser, ProblemStore};
use mycelium_project::OriginRules;

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("путь к логу");
    let packages = args.next().map(|p| vec![p]).unwrap_or_default();
    let mod_ids = args.next().map(|m| vec![m]).unwrap_or_default();
    let text = std::fs::read_to_string(&path).expect("лог читается");
    let mut parser = Parser::new(OriginRules { mod_ids, packages, ..OriginRules::default() });
    let mut store = ProblemStore::default();
    store.start_run();
    let mut entries = Vec::new();
    for line in text.lines() {
        entries.extend(parser.push(line));
    }
    entries.extend(parser.finish());
    for e in &entries {
        store.add(e);
    }
    println!("записей: {}, проблем: {}", entries.len(), store.problems().len());
    for p in store.problems() {
        let e = &p.sample;
        println!(
            "×{:<3} {:?} {:?} {:?} known={:?} noise={} thread={:?} | {}",
            p.count,
            e.level,
            e.system,
            e.origin,
            e.known,
            e.noise,
            e.thread.as_deref().unwrap_or(""),
            e.message.chars().take(110).collect::<String>()
        );
    }
}
