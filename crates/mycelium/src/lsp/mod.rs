//! Подсказки Java через jdtls (Eclipse JDT Language Server): ошибки прямо в коде,
//! автодополнение, описания при наведении и переход к определению.
//!
//! jdtls сам импортирует Gradle-проект. Метаданные Eclipse (`.project`, `.classpath`)
//! он хранит в своей рабочей папке в `%LOCALAPPDATA%`, а не в папке мода.
//! Классы Minecraft из «Перейти к определению» приходят как `jdt://…`,
//! такие ссылки открываются через индекс исходников Forma Mycelium.

pub mod install;
mod rpc;

/// Init-скрипт для импорта jdtls: вывод Eclipse — в `build/eclipse/`, а не в `bin/` мода.
const ECLIPSE_OUTPUT_SCRIPT: &str = include_str!("../gradle/eclipse_output.gradle");

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use eframe::egui;
use serde_json::{Value, json};

use crate::editors::code::{Document, Documents};
use forma_plugins::Progress;
use install::JavaHome;
use rpc::Rpc;

#[derive(Clone, Debug, PartialEq)]
pub enum LspStatus {
    /// Плагин jdtls не установлен (или выключен).
    NotInstalled,
    /// Плагин jdtls ставится.
    Installing(Progress),
    /// Нет JDK нужной версии, на котором запускается jdtls.
    NoJdk(u32),
    Starting,
    /// Импорт проекта и сборка: текст последнего шага.
    Importing(String),
    Ready,
    Failed(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

/// Ошибка или предупреждение компилятора. Позиции — строка и столбец в UTF-16, как в LSP.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    pub start: (u32, u32),
    pub end: (u32, u32),
    pub severity: Severity,
    pub message: String,
}

#[derive(Clone, Debug)]
pub struct CompletionItem {
    pub label: String,
    pub detail: Option<String>,
    pub kind: u32,
    /// Что вставить.
    pub insert: String,
    /// Начало заменяемого текста от сервера (строка, столбец UTF-16), если он его указал.
    pub replace_from: Option<(u32, u32)>,
    pub sort: String,
    pub filter: String,
    /// Исходный элемент — для `completionItem/resolve` (импорты приходят там).
    raw: Value,
}

/// Открытый список автодополнения.
#[derive(Clone, Debug)]
pub struct Completion {
    pub doc: PathBuf,
    /// Символ, с которого начинается дополняемое слово.
    pub anchor: usize,
    pub items: Vec<CompletionItem>,
    pub selected: usize,
    pub loading: bool,
    /// Сервер вернул не всё: при наборе дальше список запрашивается заново.
    pub incomplete: bool,
}

/// Описание символа при наведении.
#[derive(Clone, Debug)]
pub struct HoverInfo {
    pub doc: PathBuf,
    pub at: usize,
    pub text: String,
}

/// Правка текста от сервера (импорт после автодополнения).
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    pub start: (u32, u32),
    pub end: (u32, u32),
    pub text: String,
}

/// Что сделать приложению по ответу сервера.
#[derive(Clone, Debug, PartialEq)]
pub enum LspEvent {
    /// Открыть файл проекта на строке (с единицы).
    Open { path: PathBuf, line: u32 },
    /// Открыть класс библиотеки (из `jdt://`) на строке.
    OpenClass { class: String, line: u32 },
    /// Определение не найдено.
    NotFound,
}

enum Pending {
    Initialize,
    Completion { doc: PathBuf },
    Resolve { doc: PathBuf },
    Hover { doc: PathBuf, at: usize },
    Definition,
}

pub struct JavaLsp {
    pub status: LspStatus,
    /// Чтобы будить интерфейс, когда сервер что-то прислал.
    ctx: egui::Context,
    root: PathBuf,
    jdk_hint: Option<PathBuf>,
    rpc: Option<Rpc>,
    /// Папка плагина jdtls; `None` — плагин не установлен или выключен.
    server: Option<PathBuf>,
    initialized: bool,
    pending: HashMap<i64, Pending>,
    /// Открытые на сервере документы и отправленная версия.
    opened: HashMap<String, (PathBuf, i32)>,
    /// Ошибки по файлам (ключ — нормализованный путь).
    diagnostics: HashMap<String, Vec<Diagnostic>>,
    pub completion: Option<Completion>,
    pub hover: Option<HoverInfo>,
    pub events: Vec<LspEvent>,
    /// Правки для документов (импорты), применяются редактором на следующем кадре.
    pub edits: Vec<(PathBuf, Vec<Edit>)>,
    settings: Value,
    /// Последний шаг импорта/сборки (`$/progress`).
    progress: HashMap<String, String>,
}

impl JavaLsp {
    /// Подсказки для проекта. `server` — папка плагина jdtls: если он есть, сервер сразу запускается.
    pub fn new(ctx: &egui::Context, root: &Path, jdk_hint: Option<PathBuf>, server: Option<PathBuf>) -> Self {
        let mut lsp = Self {
            status: LspStatus::NotInstalled,
            ctx: ctx.clone(),
            root: root.to_path_buf(),
            jdk_hint,
            rpc: None,
            server,
            initialized: false,
            pending: HashMap::new(),
            opened: HashMap::new(),
            diagnostics: HashMap::new(),
            completion: None,
            hover: None,
            events: Vec::new(),
            edits: Vec::new(),
            settings: Value::Null,
            progress: HashMap::new(),
        };
        if lsp.server.is_some() {
            lsp.start();
        }
        lsp
    }

    pub fn is_ready(&self) -> bool {
        self.initialized && matches!(self.status, LspStatus::Ready | LspStatus::Importing(_))
    }

    /// Папка плагина jdtls (установлен и включён) или `None`. Сменилась — сервер перезапускается.
    pub fn set_server(&mut self, dir: Option<PathBuf>) {
        if self.server == dir {
            return;
        }
        self.stop();
        self.server = dir;
        if self.server.is_some() {
            self.start();
        } else {
            self.status = LspStatus::NotInstalled;
        }
    }

    pub fn has_server(&self) -> bool {
        self.server.is_some()
    }

    /// Шаг установки плагина jdtls, пока сервера нет.
    pub fn set_installing(&mut self, progress: Option<Progress>) {
        if self.server.is_some() {
            return;
        }
        match progress {
            Some(p) => self.status = LspStatus::Installing(p),
            None if matches!(self.status, LspStatus::Installing(_)) => self.status = LspStatus::NotInstalled,
            None => {}
        }
    }

    /// Установка плагина не удалась.
    pub fn set_install_failed(&mut self, error: String) {
        if self.server.is_none() {
            self.status = LspStatus::Failed(error);
        }
    }

    /// Перезапуск сервера (например, после правки build.gradle или зависания).
    pub fn restart(&mut self) {
        self.stop();
        self.start();
    }

    fn stop(&mut self) {
        if let Some(mut rpc) = self.rpc.take() {
            // По-хорошему: jdtls сохраняет рабочую папку. Если убить его сразу, следующий
            // запуск восстанавливает её и надолго зависает на запуске Buildship.
            if self.initialized {
                let id = rpc.request("shutdown", Value::Null);
                rpc.wait_response(id, std::time::Duration::from_secs(3));
                rpc.notify("exit", Value::Null);
                if rpc.wait_exit(std::time::Duration::from_secs(2)) {
                    log::debug!("jdtls stopped");
                }
            }
            rpc.kill();
        }
        self.initialized = false;
        self.pending.clear();
        self.opened.clear();
        self.diagnostics.clear();
        self.completion = None;
        self.hover = None;
    }

    fn start(&mut self) {
        let Some(dir) = self.server.clone() else {
            self.status = LspStatus::NotInstalled;
            return;
        };
        let Some(launcher) = install::launcher(&dir) else {
            self.status = LspStatus::NotInstalled;
            return;
        };
        // jdtls может требовать Java новее, чем у проекта: сервер — на самой подходящей,
        // а проект компилируется своим тулчейном.
        let need = install::required_java(&dir);
        let Some(jdk) = install::find_jdk(self.jdk_hint.as_deref(), need) else {
            self.status = LspStatus::NoJdk(need);
            return;
        };
        let project_jdk = self.jdk_hint.as_deref().and_then(|h| install::jdk_version(h).map(|major| JavaHome { home: h.to_path_buf(), major }));
        let ws = workspace_dir(&self.root);
        let mut cmd = std::process::Command::new(jdk.java());
        cmd.args([
            "-Declipse.application=org.eclipse.jdt.ls.core.id1",
            "-Dosgi.bundles.defaultStartLevel=4",
            "-Declipse.product=org.eclipse.jdt.ls.core.product",
            "-Dosgi.checkConfiguration=true",
            "-Dosgi.sharedConfiguration.area.readOnly=true",
            "-Dosgi.configuration.cascaded=true",
            "-Dlog.level=WARNING",
            // Без этого свойства Buildship пишет `.project`, `.classpath` и `.settings` в папку мода:
            // одноимённая настройка LSP перенаправление не включает, оно настраивается при старте.
            "-Djava.import.generatesMetadataFilesAtProjectRoot=false",
            "-Xms256m",
            "-Xmx2G",
            "-XX:+UseParallelGC",
            "-Dsun.zip.disableMemoryMapping=true",
            "--add-modules=ALL-SYSTEM",
            "--add-opens",
            "java.base/java.util=ALL-UNNAMED",
            "--add-opens",
            "java.base/java.lang=ALL-UNNAMED",
        ])
        .arg(format!("-Dosgi.sharedConfiguration.area={}", install::config_dir(&dir).display()))
        .args(if jdk.major >= 24 { &["-Djdk.xml.maxGeneralEntitySizeLimit=0", "-Djdk.xml.totalEntitySizeLimit=0"][..] } else { &[][..] })
        .arg("-jar")
        .arg(&launcher)
        .arg("-configuration")
        .arg(ws.join("config"))
        .arg("-data")
        .arg(ws.join("data"))
        .current_dir(&self.root);
        // Init-скрипт, который уводит вывод Eclipse из `bin/` в `build/eclipse/`.
        // Скрипт — в папке самого плагина jdtls, а не рядом с другими плагинами.
        let init_script = dir.join("eclipse-output.gradle");
        if let Err(e) = std::fs::write(&init_script, ECLIPSE_OUTPUT_SCRIPT) {
            log::warn!("Failed to write {}: {e}", init_script.display());
        }
        let ctx = self.ctx.clone();
        match Rpc::spawn(cmd, move || ctx.request_repaint()) {
            Ok(rpc) => {
                self.rpc = Some(rpc);
                self.status = LspStatus::Starting;
                self.settings = settings(&jdk, project_jdk.as_ref(), &init_script);
                self.initialize();
            }
            Err(e) => self.status = LspStatus::Failed(e.to_string()),
        }
    }

    fn initialize(&mut self) {
        let uri = uri_from_path(&self.root);
        let name = self.root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let params = json!({
            "processId": std::process::id(),
            "rootUri": uri,
            "workspaceFolders": [{"uri": uri, "name": name}],
            "clientInfo": {"name": "Forma Mycelium", "version": env!("CARGO_PKG_VERSION")},
            "capabilities": {
                "workspace": {"configuration": true, "workspaceFolders": true, "didChangeConfiguration": {"dynamicRegistration": false}},
                "textDocument": {
                    "synchronization": {"didSave": true, "dynamicRegistration": false},
                    "completion": {
                        "completionItem": {
                            "snippetSupport": false,
                            "documentationFormat": ["plaintext", "markdown"],
                            "resolveSupport": {"properties": ["documentation", "detail", "additionalTextEdits"]},
                        },
                        "contextSupport": true,
                    },
                    "hover": {"contentFormat": ["plaintext", "markdown"]},
                    "definition": {"linkSupport": false},
                    "publishDiagnostics": {"relatedInformation": false},
                },
                "window": {"workDoneProgress": true},
                "general": {"positionEncodings": ["utf-16"]},
            },
            "initializationOptions": {
                "bundles": [],
                "workspaceFolders": [uri],
                "settings": self.settings,
                "extendedClientCapabilities": {"classFileContentsSupport": true, "progressReportProvider": false},
            },
        });
        if let Some(rpc) = &mut self.rpc {
            let id = rpc.request("initialize", params);
            self.pending.insert(id, Pending::Initialize);
        }
    }

    /// Ошибки и предупреждения файла.
    pub fn diagnostics(&self, path: &Path) -> &[Diagnostic] {
        self.diagnostics.get(&norm(path)).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Сколько ошибок и предупреждений во всём проекте.
    pub fn problem_counts(&self) -> (usize, usize) {
        let all = self.diagnostics.values().flatten();
        all.fold((0, 0), |(e, w), d| match d.severity {
            Severity::Error => (e + 1, w),
            Severity::Warning => (e, w + 1),
            _ => (e, w),
        })
    }

    /// Каждый кадр: установка, сообщения сервера, синхронизация открытых файлов.
    pub fn poll(&mut self, docs: &Documents) -> bool {
        let mut changed = false;
        let Some(rpc) = &mut self.rpc else { return changed };
        match rpc.poll() {
            Some(messages) => {
                changed |= !messages.is_empty();
                for m in messages {
                    self.handle(m);
                }
            }
            None => {
                self.rpc = None;
                self.initialized = false;
                self.status = LspStatus::Failed("jdtls exited".into());
                return true;
            }
        }
        if self.initialized {
            self.sync(docs);
        }
        changed
    }

    fn handle(&mut self, m: Value) {
        let id = m.get("id").cloned();
        let method = m.get("method").and_then(Value::as_str).map(str::to_owned);
        match (id, method) {
            (Some(id), Some(method)) => self.server_request(id, &method, m.get("params").cloned().unwrap_or(Value::Null)),
            (None, Some(method)) => self.notification(&method, m.get("params").cloned().unwrap_or(Value::Null)),
            (Some(id), None) => {
                let Some(id) = id.as_i64() else { return };
                let Some(pending) = self.pending.remove(&id) else { return };
                if let Some(err) = m.get("error") {
                    log::warn!("lsp error: {err}");
                    if matches!(pending, Pending::Initialize) {
                        self.status = LspStatus::Failed(err.get("message").and_then(Value::as_str).unwrap_or("initialize failed").to_owned());
                    }
                    if matches!(pending, Pending::Completion { .. }) {
                        self.completion = None;
                    }
                    return;
                }
                self.response(pending, m.get("result").cloned().unwrap_or(Value::Null));
            }
            (None, None) => {}
        }
    }

    fn server_request(&mut self, id: Value, method: &str, params: Value) {
        let result = match method {
            // Настройки по разделам: отдаём раздел `java`, остальное — null.
            "workspace/configuration" => {
                let items = params.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
                Value::Array(
                    items
                        .iter()
                        .map(|it| match it.get("section").and_then(Value::as_str) {
                            Some(section) => section.split('.').try_fold(&self.settings, |v, k| v.get(k)).cloned().unwrap_or(Value::Null),
                            None => self.settings.clone(),
                        })
                        .collect(),
                )
            }
            "workspace/applyEdit" => json!({"applied": false}),
            _ => Value::Null,
        };
        if let Some(rpc) = &self.rpc {
            rpc.respond(id, result);
        }
    }

    fn notification(&mut self, method: &str, params: Value) {
        match method {
            "textDocument/publishDiagnostics" => {
                let Some(path) = params.get("uri").and_then(Value::as_str).and_then(path_from_uri) else { return };
                let list: Vec<Diagnostic> = params.get("diagnostics").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_diagnostic).collect()).unwrap_or_default();
                log::debug!("diagnostics: {} for {}", list.len(), params.get("uri").and_then(Value::as_str).unwrap_or(""));
                self.diagnostics.insert(norm(&path), list);
            }
            // Состояние jdtls: Starting → Started (проект импортирован).
            "language/status" => {
                let kind = params.get("type").and_then(Value::as_str).unwrap_or("");
                let message = params.get("message").and_then(Value::as_str).unwrap_or("").to_owned();
                match kind {
                    "Started" | "ServiceReady" => self.status = LspStatus::Ready,
                    "Error" => self.status = LspStatus::Failed(message),
                    "Starting" if self.status != LspStatus::Ready => self.status = LspStatus::Importing(message),
                    _ => {}
                }
            }
            "$/progress" => {
                let token = params.get("token").map(|t| t.to_string()).unwrap_or_default();
                let value = params.get("value").cloned().unwrap_or(Value::Null);
                match value.get("kind").and_then(Value::as_str) {
                    Some("end") => {
                        self.progress.remove(&token);
                    }
                    _ => {
                        let title = value.get("title").and_then(Value::as_str);
                        let msg = value.get("message").and_then(Value::as_str);
                        let text = match (title, msg) {
                            (Some(t), Some(m)) => format!("{t}: {m}"),
                            (Some(t), None) => t.to_owned(),
                            (None, Some(m)) => self.progress.get(&token).map(|old| format!("{}: {m}", old.split(':').next().unwrap_or(old))).unwrap_or_else(|| m.to_owned()),
                            (None, None) => return,
                        };
                        self.progress.insert(token, text);
                    }
                }
            }
            "window/logMessage" | "window/showMessage" => {
                let text = params.get("message").and_then(Value::as_str).unwrap_or("");
                log::debug!("jdtls: {text}");
            }
            _ => {}
        }
    }

    fn response(&mut self, pending: Pending, result: Value) {
        match pending {
            Pending::Initialize => {
                self.initialized = true;
                self.status = LspStatus::Importing(String::new());
                if let Some(rpc) = &self.rpc {
                    rpc.notify("initialized", json!({}));
                    rpc.notify("workspace/didChangeConfiguration", json!({"settings": self.settings}));
                }
            }
            Pending::Completion { doc } => {
                let Some(c) = self.completion.as_mut().filter(|c| c.doc == doc) else { return };
                let items = match &result {
                    Value::Array(a) => a.clone(),
                    Value::Object(o) => o.get("items").and_then(Value::as_array).cloned().unwrap_or_default(),
                    _ => Vec::new(),
                };
                c.incomplete = result.get("isIncomplete").and_then(Value::as_bool).unwrap_or(false);
                let mut items: Vec<CompletionItem> = items.into_iter().filter_map(parse_completion).collect();
                items.sort_by(|a, b| a.sort.cmp(&b.sort).then_with(|| a.label.cmp(&b.label)));
                c.items = items;
                c.selected = 0;
                c.loading = false;
            }
            Pending::Resolve { doc } => {
                let edits: Vec<Edit> = result.get("additionalTextEdits").and_then(Value::as_array).map(|a| a.iter().filter_map(parse_edit).collect()).unwrap_or_default();
                if !edits.is_empty() {
                    self.edits.push((doc, edits));
                }
            }
            Pending::Hover { doc, at } => {
                let text = hover_text(result.get("contents").unwrap_or(&Value::Null));
                self.hover = (!text.trim().is_empty()).then_some(HoverInfo { doc, at, text });
            }
            Pending::Definition => {
                log::debug!("definition: {result}");
                let loc = match &result {
                    Value::Array(a) => a.first().cloned(),
                    Value::Object(_) => Some(result.clone()),
                    _ => None,
                };
                let event = loc
                    .and_then(|l| {
                        let uri = l.get("uri").or_else(|| l.get("targetUri"))?.as_str()?.to_owned();
                        let range = l.get("range").or_else(|| l.get("targetSelectionRange"))?;
                        let line = range.pointer("/start/line")?.as_u64()? as u32 + 1;
                        if let Some(class) = class_from_jdt_uri(&uri) {
                            Some(LspEvent::OpenClass { class, line })
                        } else {
                            Some(LspEvent::Open { path: path_from_uri(&uri)?, line })
                        }
                    })
                    .unwrap_or(LspEvent::NotFound);
                self.events.push(event);
            }
        }
    }

    /// Документ — Java-файл проекта, который ведёт сервер.
    fn tracks(&self, doc: &Document) -> bool {
        doc.lang == "java" && !doc.readonly() && doc.error.is_none() && doc.path.starts_with(&self.root)
    }

    /// Открывает новые документы на сервере, отправляет изменения и закрывает ушедшие.
    fn sync(&mut self, docs: &Documents) {
        let mut alive: Vec<String> = Vec::new();
        for doc in docs.iter() {
            if self.tracks(doc) {
                self.sync_one(doc);
                alive.push(norm(&doc.path));
            }
        }
        let gone: Vec<String> = self.opened.keys().filter(|k| !alive.contains(k)).cloned().collect();
        for key in gone {
            if let Some((path, _)) = self.opened.remove(&key) {
                if let Some(rpc) = &self.rpc {
                    rpc.notify("textDocument/didClose", json!({"textDocument": {"uri": uri_from_path(&path)}}));
                }
            }
        }
    }

    fn sync_one(&mut self, doc: &Document) {
        let Some(rpc) = &self.rpc else { return };
        let key = norm(&doc.path);
        let uri = uri_from_path(&doc.path);
        match self.opened.get(&key) {
            None => {
                log::debug!("didOpen {uri}");
                rpc.notify("textDocument/didOpen", json!({"textDocument": {"uri": uri, "languageId": "java", "version": doc.version, "text": doc.text}}));
            }
            Some((_, v)) if *v != doc.version => {
                rpc.notify("textDocument/didChange", json!({"textDocument": {"uri": uri, "version": doc.version}, "contentChanges": [{"text": doc.text}]}));
            }
            _ => return,
        }
        self.opened.insert(key, (doc.path.clone(), doc.version));
    }

    fn position(&mut self, doc: &Document, at: usize) -> Option<Value> {
        if !self.initialized || !self.tracks(doc) {
            return None;
        }
        // Сервер должен знать текущий текст до запроса.
        self.sync_one(doc);
        let (line, character) = lsp_pos(&doc.text, at);
        Some(json!({"textDocument": {"uri": uri_from_path(&doc.path)}, "position": {"line": line, "character": character}}))
    }

    /// Автодополнение в позиции `at` (символ); `anchor` — начало дополняемого слова.
    pub fn request_completion(&mut self, doc: &Document, at: usize, anchor: usize) {
        let Some(params) = self.position(doc, at) else { return };
        let Some(rpc) = &mut self.rpc else { return };
        let id = rpc.request("textDocument/completion", params);
        self.pending.insert(id, Pending::Completion { doc: doc.path.clone() });
        // Повторный запрос того же слова не прячет уже показанный список.
        match self.completion.as_mut().filter(|c| c.doc == doc.path && c.anchor == anchor) {
            Some(c) => c.loading = true,
            None => self.completion = Some(Completion { doc: doc.path.clone(), anchor, items: Vec::new(), selected: 0, loading: true, incomplete: false }),
        }
    }

    /// Выбран элемент: просим у сервера импорты и прочие правки для него.
    pub fn resolve(&mut self, doc: &Path, item: &CompletionItem) {
        let Some(rpc) = &mut self.rpc else { return };
        let id = rpc.request("completionItem/resolve", item.raw.clone());
        self.pending.insert(id, Pending::Resolve { doc: doc.to_path_buf() });
    }

    pub fn request_hover(&mut self, doc: &Document, at: usize) {
        let Some(params) = self.position(doc, at) else { return };
        let Some(rpc) = &mut self.rpc else { return };
        let id = rpc.request("textDocument/hover", params);
        self.pending.insert(id, Pending::Hover { doc: doc.path.clone(), at });
    }

    pub fn request_definition(&mut self, doc: &Document, at: usize) {
        let Some(params) = self.position(doc, at) else { return };
        let Some(rpc) = &mut self.rpc else { return };
        let id = rpc.request("textDocument/definition", params);
        self.pending.insert(id, Pending::Definition);
    }

    /// Текст текущего шага импорта или сборки.
    pub fn progress(&self) -> Option<&str> {
        self.progress.values().next().map(String::as_str)
    }
}

impl Drop for JavaLsp {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Настройки jdtls (раздел `java`). `server` — JDK, на котором работает jdtls,
/// `project` — тулчейн проекта: им компилируется код и запускается Gradle.
fn settings(server: &JavaHome, project: Option<&JavaHome>, init_script: &Path) -> Value {
    let main = project.filter(|p| p.major >= 17).unwrap_or(server);
    let home = main.home.display().to_string();
    let mut runtimes = vec![json!({"name": format!("JavaSE-{}", main.major), "path": home, "default": true})];
    if server.major != main.major {
        runtimes.push(json!({"name": format!("JavaSE-{}", server.major), "path": server.home.display().to_string()}));
    }
    json!({
        "java": {
            "import": {
                // Аргументы списком: путь со пробелами остаётся одним аргументом.
                "gradle": {
                    "enabled": true,
                    "wrapper": {"enabled": true},
                    "java": {"home": home},
                    "arguments": ["--init-script", init_script.display().to_string()],
                },
                "maven": {"enabled": false},
                // `.project` и `.classpath` — в рабочей папке jdtls, а не в папке мода.
                "generatesMetadataFilesAtProjectRoot": false,
                "exclusions": ["**/node_modules/**", "**/.metadata/**", "**/run/**", "**/runs/**"],
            },
            "configuration": {
                "updateBuildConfiguration": "automatic",
                "runtimes": runtimes,
            },
            "autobuild": {"enabled": true},
            "completion": {"enabled": true, "guessMethodArguments": false, "overwrite": true},
            "format": {"enabled": false},
            "signatureHelp": {"enabled": false},
            "inlayHints": {"parameterNames": {"enabled": "none"}},
        }
    })
}

/// Рабочая папка jdtls для проекта: `%LOCALAPPDATA%\Forma\Mycelium\jdtls-workspaces\<имя>-<хеш>`.
fn workspace_dir(root: &Path) -> PathBuf {
    let name: String = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect();
    // FNV-1a: короткий устойчивый хеш пути.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in norm(root).bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let base = crate::settings::cache_dir().unwrap_or_else(std::env::temp_dir);
    base.join("jdtls-workspaces").join(format!("{name}-{:08x}", hash as u32))
}

/// Путь для сравнения: в Windows регистр и вид разделителей не важны.
pub fn norm(path: &Path) -> String {
    let s = path.to_string_lossy().replace('/', "\\");
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// `C:\mods\Мой мод\A.java` → `file:///C:/mods/%D0%9C%D0%BE%D0%B9%20%D0%BC%D0%BE%D0%B4/A.java`.
pub fn uri_from_path(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from(if s.starts_with('/') { "file://" } else { "file:///" });
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// `file:///c%3A/mods/A.java` → `c:\mods\A.java`.
pub fn path_from_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(v) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).ok()?, 16) {
                decoded.push(v);
                i += 3;
                continue;
            }
        }
        decoded.push(bytes[i]);
        i += 1;
    }
    let s = String::from_utf8(decoded).ok()?;
    if cfg!(windows) {
        // `/C:/mods` → `C:\mods`.
        let s = s.strip_prefix('/').unwrap_or(&s);
        Some(PathBuf::from(s.replace('/', "\\")))
    } else {
        Some(PathBuf::from(s))
    }
}

/// `jdt://contents/neoforge-21.1.252.jar/net.minecraft.world.item/Item.class?=…` → `net.minecraft.world.item.Item`.
pub fn class_from_jdt_uri(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("jdt://contents/")?;
    let rest = rest.split('?').next()?;
    let mut parts = rest.split('/');
    let _jar = parts.next()?;
    let package = parts.next()?;
    // `.class` — без исходников, `.java` — если jdtls подключил `-sources.jar`.
    let file = parts.next()?;
    let class = file.strip_suffix(".class").or_else(|| file.strip_suffix(".java"))?;
    let class = class.split('$').next().unwrap_or(class);
    Some(if package.is_empty() { class.to_owned() } else { format!("{package}.{class}") })
}

/// Символ `at` → строка и столбец в UTF-16.
pub fn lsp_pos(text: &str, at: usize) -> (u32, u32) {
    let mut line = 0;
    let mut col = 0;
    for c in text.chars().take(at) {
        if c == '\n' {
            line += 1;
            col = 0;
        } else {
            col += c.len_utf16() as u32;
        }
    }
    (line, col)
}

/// Строка и столбец в UTF-16 → номер символа. За концом строки — её конец.
pub fn char_index(text: &str, line: u32, col: u32) -> usize {
    let mut cur_line = 0;
    let mut cur_col = 0;
    for (i, c) in text.chars().enumerate() {
        if cur_line == line && (cur_col >= col || c == '\n') {
            return i;
        }
        if c == '\n' {
            if cur_line == line {
                return i;
            }
            cur_line += 1;
            cur_col = 0;
        } else if cur_line == line {
            cur_col += c.len_utf16() as u32;
        }
    }
    text.chars().count()
}

fn pos(v: &Value, key: &str) -> Option<(u32, u32)> {
    let p = v.get(key)?;
    Some((p.get("line")?.as_u64()? as u32, p.get("character")?.as_u64()? as u32))
}

fn parse_diagnostic(d: &Value) -> Option<Diagnostic> {
    let range = d.get("range")?;
    let severity = match d.get("severity").and_then(Value::as_u64).unwrap_or(1) {
        1 => Severity::Error,
        2 => Severity::Warning,
        3 => Severity::Info,
        _ => Severity::Hint,
    };
    Some(Diagnostic { start: pos(range, "start")?, end: pos(range, "end")?, severity, message: d.get("message")?.as_str()?.to_owned() })
}

fn parse_edit(e: &Value) -> Option<Edit> {
    let range = e.get("range")?;
    Some(Edit { start: pos(range, "start")?, end: pos(range, "end")?, text: e.get("newText")?.as_str()?.to_owned() })
}

fn parse_completion(item: Value) -> Option<CompletionItem> {
    let label = item.get("label")?.as_str()?.to_owned();
    let edit = item.get("textEdit");
    let insert = edit
        .and_then(|e| e.get("newText"))
        .or_else(|| item.get("insertText"))
        .and_then(Value::as_str)
        .unwrap_or(&label)
        .to_owned();
    // `range` у обычной правки, `insert` у правки «вставить/заменить».
    let replace_from = edit.and_then(|e| e.get("range").or_else(|| e.get("insert"))).and_then(|r| pos(r, "start"));
    Some(CompletionItem {
        detail: item.get("detail").and_then(Value::as_str).map(str::to_owned),
        kind: item.get("kind").and_then(Value::as_u64).unwrap_or(0) as u32,
        sort: item.get("sortText").and_then(Value::as_str).unwrap_or(&label).to_owned(),
        filter: item.get("filterText").and_then(Value::as_str).unwrap_or(&label).to_owned(),
        label,
        insert,
        replace_from,
        raw: item,
    })
}

/// Текст описания: MarkupContent, MarkedString или их список. Markdown-ограды кода убираются.
fn hover_text(contents: &Value) -> String {
    let piece = |v: &Value| -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Object(o) => o.get("value").and_then(Value::as_str).unwrap_or("").to_owned(),
            _ => String::new(),
        }
    };
    let text = match contents {
        Value::Array(a) => a.iter().map(piece).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("\n\n"),
        v => piece(v),
    };
    let text = text.lines().filter(|l| !l.trim_start().starts_with("```")).collect::<Vec<_>>().join("\n");
    plain_markdown(&text)
}

/// Markdown описаний jdtls → простой текст: ссылки `[текст](адрес)` → текст,
/// без звёздочек и обратных кавычек, HTML-сущности раскрыты, без лишних пустых строк.
fn plain_markdown(md: &str) -> String {
    let mut out = String::with_capacity(md.len());
    let chars: Vec<char> = md.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '[' {
            // Ищем `](` и закрывающую `)`.
            if let Some(close) = chars[i + 1..].iter().position(|&x| x == ']').map(|p| p + i + 1) {
                if chars.get(close + 1) == Some(&'(') {
                    if let Some(end) = chars[close + 2..].iter().position(|&x| x == ')').map(|p| p + close + 2) {
                        out.extend(&chars[i + 1..close]);
                        i = end + 1;
                        continue;
                    }
                }
            }
        }
        if c != '*' && c != '`' {
            out.push(c);
        }
        i += 1;
    }
    let out = out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&nbsp;", " ").replace("&amp;", "&");
    let mut lines: Vec<&str> = Vec::new();
    for l in out.lines().map(str::trim_end) {
        if l.is_empty() && lines.last().is_some_and(|p| p.is_empty()) {
            continue;
        }
        lines.push(l);
    }
    lines.join("\n").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uris_roundtrip_with_non_ascii() {
        let p = PathBuf::from(if cfg!(windows) { r"C:\Моды\my mod\A.java" } else { "/mods/my mod/A.java" });
        let uri = uri_from_path(&p);
        assert!(uri.starts_with("file:///"));
        assert!(uri.contains("my%20mod"));
        assert_eq!(norm(&path_from_uri(&uri).unwrap()), norm(&p));
        if cfg!(windows) {
            assert_eq!(norm(&path_from_uri("file:///c%3A/mods/A.java").unwrap()), norm(Path::new(r"C:\mods\A.java")));
        }
    }

    #[test]
    fn utf16_positions() {
        let text = "a😀b\nдва";
        assert_eq!(lsp_pos(text, 2), (0, 3), "эмодзи — две единицы UTF-16");
        assert_eq!(char_index(text, 0, 3), 2);
        assert_eq!(char_index(text, 1, 2), 6);
        assert_eq!(char_index(text, 0, 99), 3, "за концом строки — конец строки");
    }

    #[test]
    fn jdt_uris_name_classes() {
        let uri = "jdt://contents/neoforge-21.1.252.jar/net.minecraft.world.item/Item.class?=testmod/C:%5C/x.jar";
        assert_eq!(class_from_jdt_uri(uri).as_deref(), Some("net.minecraft.world.item.Item"));
        assert_eq!(class_from_jdt_uri("jdt://contents/a.jar/p/A$B.class").as_deref(), Some("p.A"));
        let with_sources = "jdt://contents/neoforge-21.1.252.jar/net.minecraft.world.item/Item.java?=testmod/C:%5C/x.jar=/%3Cnet.minecraft.world.item%28Item.class";
        assert_eq!(class_from_jdt_uri(with_sources).as_deref(), Some("net.minecraft.world.item.Item"));
    }

    #[test]
    fn completion_items_and_hover() {
        let item = json!({"label": "getDescriptionId() : String", "kind": 2, "sortText": "999", "textEdit": {"range": {"start": {"line": 3, "character": 10}, "end": {"line": 3, "character": 12}}, "newText": "getDescriptionId()"}});
        let c = parse_completion(item).unwrap();
        assert_eq!(c.insert, "getDescriptionId()");
        assert_eq!(c.replace_from, Some((3, 10)));
        assert_eq!(hover_text(&json!({"kind": "markdown", "value": "```java\nItem item\n```\nПредмет"})), "Item item\nПредмет");
        let md = "net.minecraft.world.item.Item\n\n\n\nSource: *[neoforge.jar](jdt://contents/x)*\n\nReturns &lt;b&gt; `code`";
        assert_eq!(plain_markdown(md), "net.minecraft.world.item.Item\n\nSource: neoforge.jar\n\nReturns <b> code");
    }
}
