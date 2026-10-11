//! Сборник проблем запусков: одинаковые ошибки схлопываются в одну строку со счётчиком,
//! а окно исключений группирует их по системам, типам, происхождению и серьёзности.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::model::{Entry, Source};

/// Числа и шестнадцатеричные адреса в тексте не делают ошибку «другой».
static NUMBERS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"0x[0-9a-fA-F]+|\d+").expect("регулярное выражение чисел"));

/// Одна проблема: все её повторы.
#[derive(Clone, Debug)]
pub struct Problem {
    pub signature: String,
    /// Последнее появление — с ним и работаем в окне исключений.
    pub sample: Entry,
    pub count: u32,
    pub first_time: Option<String>,
    pub last_time: Option<String>,
    /// В каком запуске проблема появилась впервые.
    pub first_run: u32,
    /// В каком запуске встречалась последний раз.
    pub last_run: u32,
}

impl Problem {
    /// Появилась в текущем запуске впервые.
    pub fn is_new(&self, run: u32) -> bool {
        self.first_run == run && run > 1
    }
}

#[derive(Default)]
pub struct ProblemStore {
    problems: Vec<Problem>,
    index: HashMap<String, usize>,
    run: u32,
}

impl ProblemStore {
    /// Начинается новый запуск: проблемы прошлых запусков остаются, но помечаются старыми.
    pub fn start_run(&mut self) {
        self.run += 1;
    }

    pub fn run(&self) -> u32 {
        self.run
    }

    pub fn problems(&self) -> &[Problem] {
        &self.problems
    }

    pub fn clear(&mut self) {
        self.problems.clear();
        self.index.clear();
    }

    /// Добавляет запись, если это проблема. Возвращает `true` для новой проблемы.
    pub fn add(&mut self, e: &Entry) -> bool {
        if !e.is_problem() {
            return false;
        }
        let sig = signature(e);
        let run = self.run.max(1);
        match self.index.get(&sig) {
            Some(&i) => {
                let p = &mut self.problems[i];
                p.count += 1;
                p.last_time = e.time.clone().or(p.last_time.take());
                p.last_run = run;
                p.sample = e.clone();
                false
            }
            None => {
                self.index.insert(sig.clone(), self.problems.len());
                self.problems.push(Problem {
                    signature: sig,
                    sample: e.clone(),
                    count: 1,
                    first_time: e.time.clone(),
                    last_time: e.time.clone(),
                    first_run: run,
                    last_run: run,
                });
                true
            }
        }
    }
}

/// Подпись проблемы: одинаковая у повторов одной и той же ошибки.
pub fn signature(e: &Entry) -> String {
    if let Some(loc) = &e.location {
        return format!("compile|{}|{}|{}", loc.file, loc.line, e.message);
    }
    if let Some(t) = &e.throwable {
        let root = t.root_cause();
        // Место: первый кадр своего мода во всей цепочке, иначе верхний кадр причины.
        let place = t
            .chain()
            .flat_map(|c| c.frames.iter())
            .find(|f| f.origin == mycelium_project::Origin::OwnMod)
            .or_else(|| root.frames.first())
            .or_else(|| t.frames.first())
            .map(|f| format!("{}.{}:{}", f.class, f.method, f.line.unwrap_or(0)))
            .unwrap_or_default();
        return format!("throw|{}|{}", root.type_name, place);
    }
    let text = NUMBERS.replace_all(&e.message, "#");
    let kind = e.known.map(|k| k.key()).unwrap_or(match e.source {
        Source::Gradle | Source::Compiler => "build",
        _ => "log",
    });
    format!("{kind}|{:?}|{}", e.level, text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Level;
    use crate::parse::Parser;
    use mycelium_project::OriginRules;

    fn rules() -> OriginRules {
        OriginRules { mod_ids: vec!["testmod".into()], packages: vec!["com.holo.testmod".into()], ..OriginRules::default() }
    }

    #[test]
    fn repeats_collapse_and_runs_mark_new_problems() {
        let log = "\
[11Oct2026 00:13:00.000] [Worker-Main-3/WARN] [net.minecraft.client.resources.model.ModelManager/]: Missing textures in model testmod:item/ruby#inventory:
[11Oct2026 00:13:01.000] [Worker-Main-4/WARN] [net.minecraft.client.resources.model.ModelManager/]: Missing textures in model testmod:item/ruby#inventory:
[11Oct2026 00:13:02.000] [main/INFO] [x/]: fine
[11Oct2026 00:13:03.000] [Worker-Main-4/ERROR] [x/]: Took 12 ms to fail
[11Oct2026 00:13:04.000] [Worker-Main-4/ERROR] [x/]: Took 345 ms to fail";
        let mut parser = Parser::new(rules());
        let mut store = ProblemStore::default();
        store.start_run();
        for line in log.lines() {
            for e in parser.push(line) {
                store.add(&e);
            }
        }
        for e in parser.finish() {
            store.add(&e);
        }
        assert_eq!(store.problems().len(), 2, "текстура дважды и «Took N ms» с разными числами");
        assert_eq!(store.problems()[0].count, 2);
        assert_eq!(store.problems()[0].last_time.as_deref(), Some("11Oct2026 00:13:01.000"));
        assert!(!store.problems()[0].is_new(store.run()), "первый запуск — ничего не «новое»");

        store.start_run();
        let mut e = Entry::new(99, Level::Error, Source::Game, "Brand new");
        crate::classify::classify(&mut e, &rules());
        assert!(store.add(&e));
        assert!(store.problems().last().unwrap().is_new(store.run()));
    }
}
