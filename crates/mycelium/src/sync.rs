//! Синхронизация с Gradle: init-скрипт выгружает модель проекта (модули, зависимости,
//! JDK, задачи запуска), а потом в том же фоновом потоке разбираются внешние библиотеки.
//! Интерфейс в это время не ждёт: результаты забираются из канала на каждом кадре.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::sync::{Arc, Mutex};

use mycelium_project::{GradleModel, Libraries, ModInfo, Project, model, store};

use crate::run::{gradle_command, kill_tree};

/// Init-скрипт выгрузки модели. Кладётся в `.mycelium/cache/` перед каждым запуском.
const MODEL_SCRIPT: &str = include_str!("gradle/model.gradle");
/// Сколько последних строк вывода Gradle показываем при ошибке.
const TAIL: usize = 40;

#[derive(Clone, Debug, PartialEq)]
pub enum SyncState {
    Idle,
    /// Gradle выгружает модель.
    Running { since: f64 },
    /// Модель есть, разбираются jar.
    Indexing,
    Failed { message: String },
}

pub enum SyncResult {
    Model(GradleModel),
    Libraries(Arc<Libraries>),
    Failed { message: String, output: Vec<String> },
}

/// Синхронизация открытого проекта.
pub struct GradleSync {
    pub state: SyncState,
    /// Хвост вывода Gradle при последней ошибке.
    pub output: Vec<String>,
    /// Номер проекта: результаты для ранее открытого проекта отбрасываются.
    epoch: u64,
    tx: Sender<(u64, SyncResult)>,
    rx: Receiver<(u64, SyncResult)>,
    cancel: Arc<AtomicBool>,
    running: bool,
}

impl Default for GradleSync {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self { state: SyncState::Idle, output: Vec::new(), epoch: 0, tx, rx, cancel: Arc::new(AtomicBool::new(false)), running: false }
    }
}

impl GradleSync {
    pub fn is_busy(&self) -> bool {
        matches!(self.state, SyncState::Running { .. } | SyncState::Indexing)
    }

    /// Открыт другой проект: прежняя синхронизация больше не нужна.
    pub fn reset(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.cancel = Arc::new(AtomicBool::new(false));
        self.epoch += 1;
        self.running = false;
        self.state = SyncState::Idle;
        self.output.clear();
    }

    /// Проект открыт: библиотеки из сохранённой модели сразу, а если модели нет
    /// или build-файлы менялись — заново через Gradle.
    pub fn on_open(&mut self, project: &Project, now: f64) {
        self.reset();
        if let Some(m) = &project.model {
            self.index(m.clone(), project.info.clone());
        }
        if project.info.gradle && (project.model.is_none() || GradleModel::is_stale(&project.root)) {
            self.start(project, now);
        }
    }

    /// Разбирает библиотеки модели в фоне.
    fn index(&mut self, model: GradleModel, info: ModInfo) {
        if !self.running {
            self.state = SyncState::Indexing;
        }
        let tx = self.tx.clone();
        let epoch = self.epoch;
        std::thread::spawn(move || {
            let libs = Libraries::build(&model, &info);
            let _ = tx.send((epoch, SyncResult::Libraries(Arc::new(libs))));
        });
    }

    /// Запускает выгрузку модели через Gradle.
    pub fn start(&mut self, project: &Project, now: f64) {
        if self.running || !project.info.gradle {
            return;
        }
        self.running = true;
        self.state = SyncState::Running { since: now };
        self.output.clear();
        let root = project.root.clone();
        let info = project.info.clone();
        let tx = self.tx.clone();
        let epoch = self.epoch;
        let cancel = self.cancel.clone();
        std::thread::spawn(move || {
            match export_model(&root, &cancel) {
                Ok(model) => {
                    let _ = tx.send((epoch, SyncResult::Model(model.clone())));
                    let libs = Libraries::build(&model, &info);
                    let _ = tx.send((epoch, SyncResult::Libraries(Arc::new(libs))));
                }
                Err((message, output)) => {
                    let _ = tx.send((epoch, SyncResult::Failed { message, output }));
                }
            }
        });
    }

    /// Забирает готовые результаты текущего проекта.
    pub fn poll(&mut self) -> Vec<SyncResult> {
        let mut out = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok((epoch, result)) if epoch == self.epoch => {
                    match &result {
                        SyncResult::Model(_) => self.state = SyncState::Indexing,
                        SyncResult::Libraries(_) => {
                            if !self.running || matches!(self.state, SyncState::Indexing) {
                                self.running = false;
                                self.state = SyncState::Idle;
                            }
                        }
                        SyncResult::Failed { message, output } => {
                            self.running = false;
                            self.state = SyncState::Failed { message: message.clone() };
                            self.output = output.clone();
                        }
                    }
                    out.push(result);
                }
                Ok(_) => {}
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        out
    }
}

/// Выгружает модель: `gradlew --init-script … myceliumModel`. Ошибка — сообщение и хвост вывода.
fn export_model(root: &Path, cancel: &AtomicBool) -> Result<GradleModel, (String, Vec<String>)> {
    let fail = |m: String| (m, Vec::new());
    let cache = store::dir(root).join("cache");
    std::fs::create_dir_all(&cache).map_err(|e| fail(e.to_string()))?;
    let script = cache.join("model.gradle");
    std::fs::write(&script, MODEL_SCRIPT).map_err(|e| fail(e.to_string()))?;
    let out: PathBuf = GradleModel::path(root);
    let _ = std::fs::remove_file(&out);

    let mut cmd = gradle_command(root);
    cmd.arg("--init-script")
        .arg(&script)
        .args(["--console=plain", "-q", "--no-configuration-cache"])
        .arg(format!("-Pmycelium.out={}", out.display()))
        .arg("myceliumModel");
    let mut child = cmd.spawn().map_err(|e| fail(e.to_string()))?;

    // Вывод собираем в фоне, чтобы буфер канала не переполнился и Gradle не встал.
    let lines = Arc::new(Mutex::new(Vec::<String>::new()));
    let (ltx, lrx) = channel();
    if let Some(s) = child.stdout.take() {
        crate::run::read_lines(s, ltx.clone());
    }
    if let Some(s) = child.stderr.take() {
        crate::run::read_lines(s, ltx);
    }
    let status = loop {
        while let Ok(line) = lrx.try_recv() {
            if let Some(line) = line {
                lines.lock().expect("вывод").push(line);
            }
        }
        if cancel.load(Ordering::Relaxed) {
            kill_tree(&mut child);
            return Err(fail("cancelled".into()));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(std::time::Duration::from_millis(100)),
            Err(e) => return Err(fail(e.to_string())),
        }
    };
    // Дочитываем то, что пришло после выхода.
    while let Ok(Some(line)) = lrx.recv_timeout(std::time::Duration::from_millis(200)) {
        lines.lock().expect("вывод").push(line);
    }
    let lines = std::mem::take(&mut *lines.lock().expect("вывод"));
    let tail = lines[lines.len().saturating_sub(TAIL)..].to_vec();
    if !status.success() {
        // Gradle пишет причину строкой ниже «* What went wrong:».
        let message = lines
            .iter()
            .position(|l| l.contains("What went wrong"))
            .and_then(|i| lines[i + 1..].iter().find(|l| !l.trim().is_empty()))
            .map(|l| l.trim().to_owned())
            .unwrap_or_else(|| format!("exit code {}", status.code().unwrap_or(-1)));
        return Err((message, tail));
    }
    match GradleModel::load(root) {
        Some(m) => Ok(m),
        None => Err((format!("{} was not written", model::MODEL_FILE), tail)),
    }
}
