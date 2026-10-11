//! Моды запуска проекта, как их видит Hyphae.
//!
//! `cargo run -p mycelium-hyphae --example run_mods -- <папка мода> [ещё jar…]`
//!
//! Classpath берётся из `.mycelium/cache/model.json`; jar из аргументов добавляются к нему.

use std::path::PathBuf;

use mycelium_hyphae::{RunInput, RunMods};
use mycelium_project::model::Dependency;
use mycelium_project::{GradleModel, detect};

fn main() {
    let mut args = std::env::args_os().skip(1).map(PathBuf::from);
    let Some(root) = args.next() else {
        eprintln!("usage: run_mods <mod folder> [jar…]");
        return;
    };
    let info = detect::detect(&root);
    let mut classpath: Vec<Dependency> = GradleModel::load(&root).map(|m| m.runtime_dependencies().into_iter().cloned().collect()).unwrap_or_default();
    classpath.extend(args.map(|file| Dependency { file, ..Dependency::default() }));
    let started = std::time::Instant::now();
    let run = RunMods::build(&RunInput { root: &root, info: &info, classpath: &classpath });
    println!("{} entries from {} jars in {:?}", run.mods.len(), classpath.len(), started.elapsed());
    for m in &run.mods {
        let logo = m.logo.as_ref().map(|l| format!(" logo {} B", l.len())).unwrap_or_default();
        let module = m.module.as_deref().map(|n| format!(" module {n}")).unwrap_or_default();
        println!("{:<10} {:<28} {:<40} {:<24} {:?}{logo}{module} packages {}", format!("{:?}", m.kind), m.id, m.name, m.version, m.found, m.packages.len());
    }
    for (id, place) in &run.duplicates {
        println!("DUPLICATE {id}: {place}");
    }
    let map = run.map();
    println!("map: {} packages, {} modules", map.packages.len(), map.modules.len());
}
