//! Запуск задач Gradle — клиент, сервер, генерация данных, сборка — и разбор их вывода
//! в записи консоли и проблемы для окна исключений.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use mycelium_i18n::t;
use mycelium_log::{Entry, Parser, ProblemStore};
use mycelium_project::{GradleModel, Loader, ModInfo, OriginRules};

/// Сколько записей храним в консоли; старые уходят.
const MAX_ENTRIES: usize = 100_000;
/// Сколько строк разбираем за кадр, чтобы интерфейс не замирал на лавине вывода.
const LINES_PER_FRAME: usize = 4000;
/// Если вывод замолчал на столько секунд, незаконченная запись считается готовой.
const IDLE_FLUSH: f64 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunKind {
    Client,
    Server,
    Data,
    /// Другая задача запуска (`runGameTestServer`, свои `run*` из build.gradle).
    Other,
    Build,
    Clean,
}

impl RunKind {
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
    fn of_task(task: &str) -> RunKind {
        match task {
            "runClient" => RunKind::Client,
            "runServer" => RunKind::Server,
            "runData" | "runDatagen" | "runClientData" | "runServerData" => RunKind::Data,
            _ => RunKind::Other,
        }
    }
}

/// Конфигурация запуска: задача Gradle и как её показывать.
#[derive(Clone, Debug, PartialEq)]
pub struct RunConfig {
    /// Постоянный идентификатор (сохраняется в проекте): `client`, `fabric:client`.
    pub id: String,
    pub task: String,
    pub kind: RunKind,
    /// Подпроект мультилоадерного мода (`fabric`, `neoforge`).
    pub module: Option<String>,
}

impl RunConfig {
    pub fn label(&self) -> String {
        let kind = match self.kind {
            // Своя задача — по её имени: `runGameTestServer` → `GameTestServer`.
            RunKind::Other => self.task.rsplit(':').next().unwrap_or(&self.task).trim_start_matches("run").to_owned(),
            k => k.label(),
        };
        match &self.module {
            Some(m) => format!("{m} · {kind}"),
            None => kind,
        }
    }
}

/// Задача генерации данных у загрузчика.
fn data_task(loader: Loader) -> &'static str {
    match loader {
        Loader::Fabric | Loader::Quilt => "runDatagen",
        Loader::NeoForge | Loader::Forge => "runData",
    }
}

/// Конфигурации запуска для проекта: у мультилоадерного — свои для каждого загрузчика.
/// Если Gradle уже выгрузил модель, берутся настоящие задачи `run*`, иначе — угадываются по загрузчику.
pub fn configs(info: &ModInfo, model: Option<&GradleModel>) -> Vec<RunConfig> {
    let mut out = Vec::new();
    let mut add = |id: String, task: String, kind: RunKind, module: Option<String>| out.push(RunConfig { id, task, kind, module });
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

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Idle,
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

/// Консоль запусков проекта: записи текущего запуска и проблемы всех запусков.
pub struct Console {
    pub entries: VecDeque<Entry>,
    pub status: Status,
    /// Что запущено (подпись конфигурации).
    pub label: String,
    pub problems: ProblemStore,
    /// Меняется при каждом изменении записей — по нему окна обновляют свои выборки.
    pub generation: u64,
    parser: Parser,
    process: Option<Process>,
    last_line_at: f64,
}

impl Default for Console {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            status: Status::Idle,
            label: String::new(),
            problems: ProblemStore::default(),
            generation: 0,
            parser: Parser::new(OriginRules::default()),
            process: None,
            last_line_at: 0.0,
        }
    }
}

impl Console {
    pub fn is_running(&self) -> bool {
        self.process.is_some()
    }

    /// Запускает задачу. Предыдущий запуск должен быть остановлен.
    pub fn start(&mut self, root: &Path, config: &RunConfig, rules: OriginRules, now: f64) {
        self.entries.clear();
        self.parser = Parser::new(rules);
        self.problems.start_run();
        self.label = config.label();
        self.generation += 1;
        match spawn(root, &config.task) {
            Ok(process) => {
                self.process = Some(process);
                self.status = Status::Running { since: now };
                self.last_line_at = now;
            }
            Err(err) => {
                log::warn!("Failed to start {}: {err}", config.task);
                self.status = Status::Failed(err.to_string());
            }
        }
    }

    /// Останавливает запуск вместе с дочерними процессами (Gradle останавливает и игру).
    pub fn stop(&mut self) {
        let Some(p) = self.process.as_mut() else { return };
        p.stopping = true;
        kill_tree(&mut p.child);
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.generation += 1;
    }

    /// Забирает новые строки процесса. Возвращает `true`, если что-то изменилось.
    pub fn poll(&mut self, now: f64) -> bool {
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
            self.process = None;
            changed = true;
        }
        if !done.is_empty() {
            changed = true;
            for e in done {
                self.problems.add(&e);
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

impl Drop for Console {
    fn drop(&mut self) {
        // Закрыли программу или проект — запуск не должен остаться висеть.
        self.stop();
    }
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

fn spawn(root: &Path, task: &str) -> std::io::Result<Process> {
    let mut cmd = gradle_command(root);
    cmd.args([task, "--console=plain"]);
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
        let ids: Vec<_> = configs(&single, None).into_iter().map(|c| (c.id, c.task)).collect();
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
        let c = configs(&multi, None);
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
        let c = configs(&info, Some(&model));
        let ids: Vec<&str> = c.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["client", "server", "data", "rungametestserver", "build", "clean"]);
        assert_eq!(c[3].label(), "GameTestServer");
        assert_eq!(c[3].kind, RunKind::Other);
    }

    #[test]
    fn plain_folder_has_no_configs() {
        assert!(configs(&ModInfo::default(), None).is_empty());
    }
}
