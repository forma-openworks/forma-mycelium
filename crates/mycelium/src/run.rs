//! Запуск задач Gradle — клиент, сервер, генерация данных, сборка, свои задачи — и разбор
//! их вывода в записи консоли и проблемы для окна исключений.
//!
//! Несколько конфигураций могут работать одновременно (например, клиент и сервер, чтобы
//! проверить мод в мультиплеере): у каждого запуска свой лог, проблемы — общие.
//!
//! Конфигурации обнаруживаются по проекту (задачи `run*` из модели Gradle), а свои
//! и правки обнаруженных хранятся в `.mycelium/runs.json` — его можно класть в git.

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use mycelium_i18n::t;
use mycelium_log::{Entry, Parser, ProblemStore};
use mycelium_project::{GradleModel, Loader, ModInfo, OriginRules, store};
use serde::{Deserialize, Serialize};

/// Сколько записей храним в логе одного запуска; старые уходят.
const MAX_ENTRIES: usize = 100_000;
/// Сколько строк разбираем за кадр, чтобы интерфейс не замирал на лавине вывода.
const LINES_PER_FRAME: usize = 4000;
/// Если вывод замолчал на столько секунд, незаконченная запись считается готовой.
const IDLE_FLUSH: f64 = 0.4;
/// Файл конфигураций запуска в `.mycelium`.
pub const RUNS_FILE: &str = "runs.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunKind {
    Client,
    Server,
    Data,
    /// Другая задача запуска (`runGameTestServer`, свои задачи из build.gradle).
    Other,
    Build,
    Clean,
}

impl RunKind {
    pub const ALL: [RunKind; 6] = [RunKind::Client, RunKind::Server, RunKind::Data, RunKind::Build, RunKind::Clean, RunKind::Other];

    pub fn label(self) -> String {
        match self {
            RunKind::Client => t!("run-client"),
            RunKind::Server => t!("run-server"),
            RunKind::Data => t!("run-data"),
            RunKind::Other => t!("run-other"),
            RunKind::Build => t!("run-build"),
            RunKind::Clean => t!("run-clean"),
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            RunKind::Client => "🎮",
            RunKind::Server => "🖧",
            RunKind::Data => "🗃",
            RunKind::Other => "▶",
            RunKind::Build => "🔨",
            RunKind::Clean => "🧹",
        }
    }

    /// Вид запуска по задаче Gradle: `runClient`, `runDatagen`, `runGameTestServer`…
    pub fn of_task(task: &str) -> RunKind {
        let last = task.split_whitespace().last().unwrap_or(task);
        let name = last.rsplit(':').next().unwrap_or(last);
        match name {
            "runClient" => RunKind::Client,
            "runServer" => RunKind::Server,
            "runData" | "runDatagen" | "runClientData" | "runServerData" => RunKind::Data,
            "build" | "assemble" | "jar" => RunKind::Build,
            "clean" => RunKind::Clean,
            _ => RunKind::Other,
        }
    }
}

/// Конфигурация запуска: задача Gradle, её аргументы и как её показывать.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunConfig {
    /// Постоянный идентификатор: `client`, `fabric:client`, `custom-3`.
    pub id: String,
    /// Своё название; без него — по виду и модулю («neoforge · Клиент»).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Задачи Gradle через пробел: `runClient`, `:fabric:runClient`, `clean build`.
    pub task: String,
    /// Дополнительные аргументы Gradle: `--offline --stacktrace -Pfoo=bar`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub args: String,
    pub kind: RunKind,
    /// Подпроект мультилоадерного мода (`fabric`, `neoforge`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,
    /// Найдена в проекте сама (а не добавлена вручную).
    #[serde(skip)]
    pub detected: bool,
}

impl RunConfig {
    fn detected(id: String, task: String, kind: RunKind, module: Option<String>) -> Self {
        Self { id, name: None, task, args: String::new(), kind, module, detected: true }
    }

    pub fn label(&self) -> String {
        if let Some(name) = self.name.as_ref().filter(|n| !n.trim().is_empty()) {
            return name.clone();
        }
        let kind = match self.kind {
            // Своя задача — по её имени: `runGameTestServer` → `GameTestServer`.
            RunKind::Other => {
                let last = self.task.split_whitespace().last().unwrap_or(&self.task);
                let name = last.rsplit(':').next().unwrap_or(last);
                name.strip_prefix("run").filter(|s| !s.is_empty()).unwrap_or(name).to_owned()
            }
            k => k.label(),
        };
        match &self.module {
            Some(m) => format!("{m} · {kind}"),
            None => kind,
        }
    }

    /// Командная строка для подсказки: `gradlew runClient --offline`.
    pub fn command_line(&self) -> String {
        if self.args.trim().is_empty() { format!("gradlew {}", self.task) } else { format!("gradlew {} {}", self.task, self.args.trim()) }
    }
}

/// `.mycelium/runs.json`: свои конфигурации, правки найденных и скрытые найденные.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunsFile {
    pub version: u32,
    pub custom: Vec<RunConfig>,
    pub overrides: BTreeMap<String, RunConfig>,
    pub hidden: Vec<String>,
}

impl RunsFile {
    pub fn load(root: &Path) -> Self {
        store::read_json(root, RUNS_FILE).unwrap_or_default()
    }

    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        let mut file = self.clone();
        file.version = 1;
        store::write_json(root, RUNS_FILE, &file)
    }
}

/// Задача генерации данных у загрузчика.
fn data_task(loader: Loader) -> &'static str {
    match loader {
        Loader::Fabric | Loader::Quilt => "runDatagen",
        Loader::NeoForge | Loader::Forge => "runData",
    }
}

/// Конфигурации, найденные в проекте: у мультилоадерного — свои для каждого загрузчика.
/// Если Gradle уже выгрузил модель, берутся настоящие задачи `run*`, иначе — угадываются по загрузчику.
pub fn detected_configs(info: &ModInfo, model: Option<&GradleModel>) -> Vec<RunConfig> {
    let mut out = Vec::new();
    let mut add = |id: String, task: String, kind: RunKind, module: Option<String>| out.push(RunConfig::detected(id, task, kind, module));
    let loader_modules: Vec<_> = info.modules.iter().filter(|m| m.loader.is_some() && !m.path.as_os_str().is_empty()).collect();
    let from_model: Vec<_> = model.map(|m| m.projects.iter().filter(|p| !p.run_tasks.is_empty()).collect()).unwrap_or_default();
    if !from_model.is_empty() {
        // Сначала клиент, сервер, данные, потом остальное — в порядке, привычном по IDEA.
        let order = |t: &str| match RunKind::of_task(t) {
            RunKind::Client => 0,
            RunKind::Server => 1,
            RunKind::Data => 2,
            _ => 3,
        };
        for p in from_model {
            let module = (p.path != ":").then(|| p.path.trim_start_matches(':').replace(':', "/"));
            let mut tasks = p.run_tasks.clone();
            tasks.sort_by_key(|t| order(t));
            for task in tasks {
                let kind = RunKind::of_task(&task);
                let short = match kind {
                    RunKind::Client => "client".to_owned(),
                    RunKind::Server => "server".to_owned(),
                    RunKind::Data => "data".to_owned(),
                    _ => task.to_lowercase(),
                };
                let (id, gradle_task) = match &module {
                    Some(m) => (format!("{m}:{short}"), format!("{}:{task}", p.path)),
                    None => (short, task),
                };
                add(id, gradle_task, kind, module.clone());
            }
        }
    } else if info.is_multiloader() && !loader_modules.is_empty() {
        for m in loader_modules {
            let name = m.path.to_string_lossy().replace('\\', "/");
            let gradle = format!(":{}", name.replace('/', ":"));
            let loader = m.loader.expect("отфильтровано выше");
            add(format!("{name}:client"), format!("{gradle}:runClient"), RunKind::Client, Some(name.clone()));
            add(format!("{name}:server"), format!("{gradle}:runServer"), RunKind::Server, Some(name.clone()));
            add(format!("{name}:data"), format!("{gradle}:{}", data_task(loader)), RunKind::Data, Some(name.clone()));
        }
    } else if let Some(&loader) = info.loaders.first() {
        add("client".into(), "runClient".into(), RunKind::Client, None);
        add("server".into(), "runServer".into(), RunKind::Server, None);
        add("data".into(), data_task(loader).into(), RunKind::Data, None);
    }
    if info.gradle {
        add("build".into(), "build".into(), RunKind::Build, None);
        add("clean".into(), "clean".into(), RunKind::Clean, None);
    }
    out
}

/// Итоговый список: найденные (с правками, без скрытых) и свои.
pub fn configs(info: &ModInfo, model: Option<&GradleModel>, file: &RunsFile) -> Vec<RunConfig> {
    let mut out: Vec<RunConfig> = detected_configs(info, model)
        .into_iter()
        .filter(|c| !file.hidden.contains(&c.id))
        .map(|c| match file.overrides.get(&c.id) {
            Some(o) => RunConfig { id: c.id, detected: true, ..o.clone() },
            None => c,
        })
        .collect();
    out.extend(file.custom.iter().cloned().map(|c| RunConfig { detected: false, ..c }));
    out
}

/// Делит строку аргументов по пробелам; кавычки объединяют слова: `-Pname="Мой мод"`.
pub fn split_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for c in s.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (None, '"' | '\'') => {
                quote = Some(c);
                has = true;
            }
            (None, c) if c.is_whitespace() => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (_, c) => {
                cur.push(c);
                has = true;
            }
        }
    }
    if has {
        out.push(cur);
    }
    out
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Running { since: f64 },
    Finished { code: Option<i32>, seconds: f64 },
    Stopped { seconds: f64 },
    /// Процесс не запустился.
    Failed(String),
}

struct Process {
    child: Child,
    rx: Receiver<Option<String>>,
    /// Сколько потоков чтения (stdout, stderr) ещё работают.
    readers: usize,
    stopping: bool,
}

/// Один запуск конфигурации: свой лог и процесс.
pub struct Session {
    pub id: u64,
    pub config: RunConfig,
    pub entries: VecDeque<Entry>,
    pub status: Status,
    /// Меняется при каждом изменении записей — по нему окна обновляют свои выборки.
    pub generation: u64,
    /// Когда запущен (секунды Unix) — для таймера в Discord.
    pub started_unix: u64,
    /// Упал: краш игры или ошибка сборки.
    pub crashed: bool,
    root: PathBuf,
    parser: Parser,
    process: Option<Process>,
    last_line_at: f64,
    /// Перезапуск: дождаться остановки и запустить снова.
    restart: bool,
}

impl Session {
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }

    fn launch(&mut self, rules: OriginRules, now: f64) {
        self.entries.clear();
        self.parser = Parser::new(rules);
        self.crashed = false;
        self.generation += 1;
        self.started_unix = unix_now();
        match spawn(&self.root, &self.config) {
            Ok(process) => {
                self.process = Some(process);
                self.status = Status::Running { since: now };
                self.last_line_at = now;
            }
            Err(err) => {
                log::warn!("Failed to start {}: {err}", self.config.task);
                self.status = Status::Failed(err.to_string());
            }
        }
    }

    fn stop(&mut self) {
        let Some(p) = self.process.as_mut() else { return };
        p.stopping = true;
        kill_tree(&mut p.child);
    }

    /// Забирает новые строки процесса. Возвращает `true`, если что-то изменилось.
    fn poll(&mut self, problems: &mut ProblemStore, now: f64) -> bool {
        let Some(p) = self.process.as_mut() else { return false };
        let mut lines = Vec::new();
        while lines.len() < LINES_PER_FRAME {
            match p.rx.try_recv() {
                Ok(Some(line)) => lines.push(line),
                Ok(None) => p.readers = p.readers.saturating_sub(1),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    p.readers = 0;
                    break;
                }
            }
        }
        let mut changed = !lines.is_empty();
        if changed {
            self.last_line_at = now;
        }
        let mut done = Vec::new();
        for line in &lines {
            done.extend(self.parser.push(line));
        }
        // Вывод замолчал — дописываем незаконченную запись, чтобы она появилась на экране.
        if lines.is_empty() && self.parser.pending().is_some() && now - self.last_line_at > IDLE_FLUSH {
            done.extend(self.parser.finish());
        }

        let p = self.process.as_mut().expect("процесс проверен выше");
        let exited = if p.readers == 0 { p.child.try_wait().ok().flatten() } else { None };
        if let Some(status) = exited {
            done.extend(self.parser.finish());
            let since = match self.status {
                Status::Running { since } => since,
                _ => now,
            };
            let seconds = now - since;
            self.status = if p.stopping { Status::Stopped { seconds } } else { Status::Finished { code: status.code(), seconds } };
            if matches!(self.status, Status::Finished { code: Some(c), .. } if c != 0) {
                self.crashed = true;
            }
            self.process = None;
            changed = true;
        }
        if !done.is_empty() {
            changed = true;
            for e in done {
                if e.crash {
                    self.crashed = true;
                }
                problems.add(&e);
                self.entries.push_back(e);
            }
            while self.entries.len() > MAX_ENTRIES {
                self.entries.pop_front();
            }
        }
        if changed {
            self.generation += 1;
        }
        changed
    }
}

/// Запуски проекта и проблемы всех запусков.
#[derive(Default)]
pub struct Console {
    pub sessions: Vec<Session>,
    pub problems: ProblemStore,
    /// Последний запущенный — консоль переключается на него.
    pub latest: u64,
    next_id: u64,
}

impl Console {
    pub fn is_running(&self) -> bool {
        self.sessions.iter().any(Session::is_running)
    }

    pub fn running(&self) -> impl Iterator<Item = &Session> {
        self.sessions.iter().filter(|s| s.is_running())
    }

    pub fn session(&self, id: u64) -> Option<&Session> {
        self.sessions.iter().find(|s| s.id == id)
    }

    /// Запуск этой конфигурации (последний, если их было несколько).
    pub fn session_of(&self, config: &str) -> Option<&Session> {
        self.sessions.iter().rev().find(|s| s.config.id == config)
    }

    pub fn is_running_config(&self, config: &str) -> bool {
        self.sessions.iter().any(|s| s.config.id == config && s.is_running())
    }

    /// Запускает конфигурацию. Уже работающая — перезапускается; законченный запуск
    /// той же конфигурации уступает место новому (его вкладка переиспользуется).
    pub fn start(&mut self, root: &Path, config: &RunConfig, rules: OriginRules, now: f64) {
        if let Some(s) = self.sessions.iter_mut().find(|s| s.config.id == config.id && s.is_running()) {
            s.config = config.clone();
            s.restart = true;
            s.stop();
            return;
        }
        self.problems.start_run();
        let id = match self.sessions.iter_mut().find(|s| s.config.id == config.id) {
            Some(s) => {
                s.config = config.clone();
                s.root = root.to_path_buf();
                s.launch(rules, now);
                s.id
            }
            None => {
                self.next_id += 1;
                let mut s = Session {
                    id: self.next_id,
                    config: config.clone(),
                    entries: VecDeque::new(),
                    status: Status::Running { since: now },
                    generation: 0,
                    started_unix: unix_now(),
                    crashed: false,
                    root: root.to_path_buf(),
                    parser: Parser::new(rules.clone()),
                    process: None,
                    last_line_at: now,
                    restart: false,
                };
                s.launch(rules, now);
                self.sessions.push(s);
                self.next_id
            }
        };
        self.latest = id;
    }

    /// Останавливает запуск вместе с дочерними процессами (Gradle останавливает и игру).
    pub fn stop(&mut self, id: u64) {
        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
            s.restart = false;
            s.stop();
        }
    }

    pub fn stop_all(&mut self) {
        for s in &mut self.sessions {
            s.restart = false;
            s.stop();
        }
    }

    /// Перезапуск: остановить и запустить снова, когда процесс завершится.
    pub fn restart(&mut self, id: u64, rules: OriginRules, now: f64) {
        let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) else { return };
        if s.is_running() {
            s.restart = true;
            s.stop();
        } else {
            self.problems.start_run();
            s.launch(rules, now);
            self.latest = id;
        }
    }

    /// Убирает законченный запуск вместе с его логом.
    pub fn close(&mut self, id: u64) {
        self.sessions.retain(|s| s.id != id || s.is_running());
    }

    pub fn clear(&mut self, id: u64) {
        if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
            s.entries.clear();
            s.generation += 1;
        }
    }

    /// Забирает вывод всех запусков и перезапускает остановленные для перезапуска.
    pub fn poll(&mut self, rules: &OriginRules, now: f64) -> bool {
        let mut changed = false;
        for s in &mut self.sessions {
            changed |= s.poll(&mut self.problems, now);
        }
        let restart: Vec<u64> = self.sessions.iter().filter(|s| s.restart && !s.is_running()).map(|s| s.id).collect();
        for id in restart {
            if let Some(s) = self.sessions.iter_mut().find(|s| s.id == id) {
                s.restart = false;
                self.problems.start_run();
                s.launch(rules.clone(), now);
                self.latest = id;
                changed = true;
            }
        }
        changed
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        // Закрыли программу или проект — запуски не должны остаться висеть.
        self.stop_all();
    }
}

pub fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Команда Gradle проекта: обёртка `gradlew`, если есть, иначе `gradle` из PATH.
/// Вывод перенаправлен, окно консоли не появляется.
pub fn gradle_command(root: &Path) -> Command {
    let wrapper = root.join(if cfg!(windows) { "gradlew.bat" } else { "gradlew" });
    let mut cmd = if wrapper.is_file() { Command::new(&wrapper) } else { Command::new("gradle") };
    cmd.current_dir(root).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // Без чёрного окна консоли поверх IDE.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn spawn(root: &Path, config: &RunConfig) -> std::io::Result<Process> {
    let mut cmd = gradle_command(root);
    cmd.args(config.task.split_whitespace()).args(split_args(&config.args)).arg("--console=plain");
    let mut child = cmd.spawn()?;
    let (tx, rx) = channel();
    let out = child.stdout.take().expect("stdout перенаправлен");
    let err = child.stderr.take().expect("stderr перенаправлен");
    read_lines(out, tx.clone());
    read_lines(err, tx);
    Ok(Process { child, rx, readers: 2, stopping: false })
}

/// Читает строки потока в фоне; в конце отправляет `None`.
pub fn read_lines(stream: impl Read + Send + 'static, tx: Sender<Option<String>>) {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_owned();
                    if tx.send(Some(line)).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = tx.send(None);
    });
}

/// Завершает процесс со всеми потомками.
pub fn kill_tree(child: &mut Child) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let killed = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status()
            .is_ok_and(|s| s.success());
        if killed {
            return;
        }
    }
    let _ = child.kill();
}

#[cfg(test)]
mod tests {
    use super::*;
    use mycelium_project::detect::Module;

    #[test]
    fn configs_for_single_and_multiloader() {
        let single = ModInfo { gradle: true, loaders: vec![Loader::Fabric], ..ModInfo::default() };
        let ids: Vec<_> = detected_configs(&single, None).into_iter().map(|c| (c.id, c.task)).collect();
        assert_eq!(ids[2], ("data".to_owned(), "runDatagen".to_owned()));
        assert_eq!(ids.len(), 5);

        let multi = ModInfo {
            gradle: true,
            loaders: vec![Loader::Fabric, Loader::NeoForge],
            modules: vec![
                Module { path: "common".into(), ..Module::default() },
                Module { path: "fabric".into(), loader: Some(Loader::Fabric), ..Module::default() },
                Module { path: "neoforge".into(), loader: Some(Loader::NeoForge), ..Module::default() },
            ],
            ..ModInfo::default()
        };
        let c = detected_configs(&multi, None);
        assert_eq!(c[0].task, ":fabric:runClient");
        assert_eq!(c[5].task, ":neoforge:runData");
        assert_eq!(c[5].module.as_deref(), Some("neoforge"));
    }

    #[test]
    fn configs_from_gradle_model_tasks() {
        let info = ModInfo { gradle: true, loaders: vec![Loader::NeoForge], ..ModInfo::default() };
        let model = GradleModel {
            projects: vec![mycelium_project::model::GradleProject {
                path: ":".into(),
                run_tasks: vec!["runData".into(), "runGameTestServer".into(), "runClient".into(), "runServer".into()],
                ..Default::default()
            }],
            ..Default::default()
        };
        let c = detected_configs(&info, Some(&model));
        let ids: Vec<&str> = c.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["client", "server", "data", "rungametestserver", "build", "clean"]);
        assert_eq!(c[3].label(), "GameTestServer");
        assert_eq!(c[3].kind, RunKind::Other);
    }

    #[test]
    fn runs_file_overrides_hides_and_adds() {
        let info = ModInfo { gradle: true, loaders: vec![Loader::NeoForge], ..ModInfo::default() };
        let mut file = RunsFile::default();
        file.hidden.push("clean".into());
        file.overrides.insert(
            "client".into(),
            RunConfig { name: Some("Клиент офлайн".into()), args: "--offline".into(), ..RunConfig::detected("client".into(), "runClient".into(), RunKind::Client, None) },
        );
        file.custom.push(RunConfig { id: "custom-1".into(), name: Some("Проверка".into()), task: "check".into(), args: String::new(), kind: RunKind::Other, module: None, detected: false });
        let c = configs(&info, None, &file);
        let ids: Vec<&str> = c.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["client", "server", "data", "build", "custom-1"]);
        assert_eq!(c[0].label(), "Клиент офлайн");
        assert!(c[0].detected && !c[4].detected);
        assert_eq!(c[0].command_line(), "gradlew runClient --offline");

        // Файл переживает сохранение и чтение.
        let json = serde_json::to_string(&file).unwrap();
        let back: RunsFile = serde_json::from_str(&json).unwrap();
        assert_eq!(back.custom[0].task, "check");
        assert_eq!(back.overrides["client"].args, "--offline");
    }

    #[test]
    fn arguments_split_with_quotes() {
        assert_eq!(split_args("--offline  -Pname=\"Мой мод\" --stacktrace"), vec!["--offline", "-Pname=Мой мод", "--stacktrace"]);
        assert_eq!(split_args("-P'a b'c \"\""), vec!["-Pa bc", ""]);
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn task_kinds() {
        assert_eq!(RunKind::of_task(":fabric:runClient"), RunKind::Client);
        assert_eq!(RunKind::of_task("clean build"), RunKind::Build);
        assert_eq!(RunKind::of_task("runGameTestServer"), RunKind::Other);
    }

    #[test]
    fn plain_folder_has_no_configs() {
        assert!(detected_configs(&ModInfo::default(), None).is_empty());
    }
}
