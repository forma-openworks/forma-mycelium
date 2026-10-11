//! Активность в Discord (Rich Presence): над каким модом идёт работа и что происходит —
//! пишется код, запущены клиент и сервер, генерируются данные, разбирается краш.
//!
//! Discord принимает картинки только по ссылке, поэтому иконка IDE и значки загрузчиков
//! лежат в публичном репозитории Forma Mycelium (`assets/discord/`), а иконка мода
//! берётся из его публичного GitHub-репозитория, если она там есть.
//!
//! Тексты активности всегда на английском, какой бы язык ни был выбран в IDE:
//! их читают друзья в Discord, а не только сам моддер.

mod ipc;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::{Duration, Instant};

use mycelium_project::{Loader, Project};
use serde_json::{Value, json};

use crate::run::RunKind;

/// Приложение «Forma Mycelium» в Discord Developer Portal.
pub const CLIENT_ID: &str = "1558787805151367198";
/// Публичные картинки для Discord.
const ASSETS: &str = "https://raw.githubusercontent.com/forma-openworks/forma-mycelium/main/assets/discord";
const IDE_REPO: &str = "https://github.com/forma-openworks/forma-mycelium";
/// Discord разрешает 5 обновлений за 20 секунд — обновляем не чаще раза в 4 секунды.
const MIN_INTERVAL: Duration = Duration::from_secs(4);
/// Discord не запущен — пробуем подключиться снова через столько.
const RECONNECT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Activity {
    pub details: String,
    pub state: String,
    /// Начало отсчёта таймера (секунды Unix).
    pub start: Option<u64>,
    pub large_image: String,
    pub large_text: String,
    pub small_image: Option<String>,
    pub small_text: Option<String>,
    pub buttons: Vec<(String, String)>,
}

/// Discord требует от 2 до 128 символов в строке.
fn fit(s: &str, max: usize) -> String {
    let mut out: String = s.chars().take(max).collect();
    while out.chars().count() < 2 {
        out.push(' ');
    }
    out
}

impl Activity {
    fn to_json(&self) -> Value {
        let mut assets = json!({"large_image": self.large_image, "large_text": fit(&self.large_text, 128)});
        if let (Some(img), Some(text)) = (&self.small_image, &self.small_text) {
            assets["small_image"] = json!(img);
            assets["small_text"] = json!(fit(text, 128));
        }
        let mut v = json!({"details": fit(&self.details, 128), "state": fit(&self.state, 128), "assets": assets, "instance": false});
        if let Some(start) = self.start {
            v["timestamps"] = json!({"start": start});
        }
        if !self.buttons.is_empty() {
            v["buttons"] = Value::Array(self.buttons.iter().take(2).map(|(label, url)| json!({"label": fit(label, 32), "url": url})).collect());
        }
        v
    }
}

/// Соединение с Discord в фоне: переподключение, ограничение частоты, повтор последнего.
pub struct Presence {
    tx: Sender<Option<Activity>>,
    /// Что отправлено в поток в последний раз — одинаковое не пересылаем.
    last: Option<Option<Activity>>,
}

impl Default for Presence {
    fn default() -> Self {
        let (tx, rx) = channel();
        std::thread::Builder::new().name("discord".into()).spawn(move || worker(rx)).expect("поток Discord запускается");
        Self { tx, last: None }
    }
}

impl Presence {
    /// Желаемая активность; `None` — убрать.
    pub fn set(&mut self, activity: Option<Activity>) {
        if self.last.as_ref() != Some(&activity) {
            let _ = self.tx.send(activity.clone());
            self.last = Some(activity);
        }
    }
}

fn worker(rx: Receiver<Option<Activity>>) {
    let mut conn: Option<ipc::Ipc> = None;
    let mut desired: Option<Activity> = None;
    // Что уже показано в Discord (`None` — ещё ничего не отправляли в это соединение).
    let mut shown: Option<Option<Activity>> = None;
    let mut last_send = Instant::now() - MIN_INTERVAL;
    let mut next_connect = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(a) => desired = a,
            Err(RecvTimeoutError::Timeout) => {}
            // Программа закрывается: убираем активность.
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(c) = conn.as_mut() {
                    let _ = c.set_activity(None);
                }
                return;
            }
        }
        while let Ok(a) = rx.try_recv() {
            desired = a;
        }
        let now = Instant::now();
        if conn.is_none() && desired.is_some() && now >= next_connect {
            match ipc::Ipc::connect(CLIENT_ID) {
                Ok(c) => {
                    conn = Some(c);
                    shown = None;
                }
                Err(e) => {
                    log::debug!("discord: {e}");
                    next_connect = now + RECONNECT;
                }
            }
        }
        let Some(c) = conn.as_mut() else { continue };
        if shown.as_ref() == Some(&desired) || now.duration_since(last_send) < MIN_INTERVAL {
            continue;
        }
        last_send = now;
        match c.set_activity(desired.as_ref().map(Activity::to_json)) {
            Ok(reply) => {
                log::debug!("discord: activity set: {}", reply.get("data").map(Value::to_string).unwrap_or_default());
                shown = Some(desired.clone());
            }
            Err(e) => {
                log::debug!("discord: {e}");
                conn = None;
                next_connect = now + RECONNECT;
            }
        }
    }
}

/// Ссылки мода в публичном GitHub-репозитории: иконка и сам репозиторий.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModLinks {
    pub icon: Option<String>,
    pub repo: Option<String>,
}

/// Что происходит в IDE — из этого собирается активность.
pub struct PresenceInput<'a> {
    pub project: Option<&'a Project>,
    /// Виды идущих запусков и название «другой» задачи.
    pub running: Vec<(RunKind, String)>,
    /// Последний законченный запуск упал, и с тех пор ничего не запускали.
    pub crashed: Option<RunKind>,
    pub started_unix: u64,
    pub away: bool,
    pub secret: bool,
    pub links: Option<&'a ModLinks>,
}

fn loader_badge(loader: Loader) -> &'static str {
    match loader {
        Loader::NeoForge => "neoforge",
        Loader::Fabric => "fabric",
        Loader::Forge => "forge",
        Loader::Quilt => "quilt",
    }
}

/// Строка статуса: что сейчас делается.
pub fn status(input: &PresenceInput) -> String {
    let kinds: Vec<RunKind> = input.running.iter().map(|(k, _)| *k).collect();
    let has = |k: RunKind| kinds.contains(&k);
    let what = if has(RunKind::Client) && has(RunKind::Server) {
        "running client & server".to_owned()
    } else if has(RunKind::Client) {
        "running the client".to_owned()
    } else if has(RunKind::Server) {
        "running the server".to_owned()
    } else if has(RunKind::Data) {
        "generating data".to_owned()
    } else if has(RunKind::Build) || has(RunKind::Clean) {
        "building the mod".to_owned()
    } else if let Some((_, name)) = input.running.iter().find(|(k, _)| *k == RunKind::Other) {
        format!("running {name}")
    } else if let Some(k) = input.crashed {
        if matches!(k, RunKind::Build | RunKind::Clean | RunKind::Other) { "fixing the build 🔧".to_owned() } else { "fixing a crash 💥".to_owned() }
    } else if input.away {
        "on a break ☕".to_owned()
    } else {
        "writing code".to_owned()
    };
    format!("Status: {what}")
}

/// Активность по состоянию IDE.
pub fn activity(input: &PresenceInput) -> Activity {
    let ide_icon = format!("{ASSETS}/mycelium.png");
    let Some(project) = input.project else {
        return Activity {
            details: "Forma Mycelium".into(),
            state: "Picking a project".into(),
            start: Some(input.started_unix),
            large_image: ide_icon,
            large_text: "Forma Mycelium".into(),
            buttons: vec![("Forma Mycelium".into(), IDE_REPO.into())],
            ..Activity::default()
        };
    };
    let info = &project.info;
    let name = info.mod_name.clone().or_else(|| info.mod_id.clone()).unwrap_or_else(|| project.name().to_owned());
    let details = match (&info.mod_name, &info.mod_id) {
        _ if input.secret => "Mod: a secret project 🤫".to_owned(),
        (Some(n), Some(id)) if n != id => format!("Mod: {n} ({id})"),
        _ => format!("Mod: {name}"),
    };
    // Большая картинка: иконка мода из его репозитория, иначе иконка IDE.
    let links = input.links.filter(|_| !input.secret);
    let (large_image, large_text) = match links.and_then(|l| l.icon.clone()) {
        Some(icon) => (icon, match &info.mod_version {
            Some(v) => format!("{name} {v}"),
            None => name.clone(),
        }),
        None => (ide_icon, "Forma Mycelium".to_owned()),
    };
    // Маленький значок — загрузчик и версии.
    let loader = info.loaders.first().copied();
    let small_text = loader.map(|l| {
        let mut s = match (&info.loader_version, info.is_multiloader()) {
            (Some(v), false) => format!("{} {v}", l.name()),
            _ => info.loaders.iter().map(|l| l.name()).collect::<Vec<_>>().join(" + "),
        };
        if let Some(mc) = &info.minecraft {
            s.push_str(&format!(" · Minecraft {mc}"));
        }
        s
    });
    let mut buttons = Vec::new();
    if let Some(repo) = links.and_then(|l| l.repo.clone()) {
        buttons.push(("Mod source".into(), repo));
    }
    buttons.push(("Forma Mycelium".into(), IDE_REPO.into()));
    Activity {
        details,
        state: status(input),
        start: Some(input.started_unix),
        large_image,
        large_text,
        small_image: loader.map(|l| format!("{ASSETS}/{}.png", loader_badge(l))),
        small_text,
        buttons,
    }
}

/// Команда без окна консоли.
fn quiet(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let out = quiet("git").arg("-C").arg(root).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned()).filter(|s| !s.is_empty())
}

/// `https://github.com/owner/repo.git`, `git@github.com:owner/repo.git` → `owner/repo`.
pub fn github_slug(remote: &str) -> Option<String> {
    let rest = remote
        .strip_prefix("https://github.com/")
        .or_else(|| remote.strip_prefix("http://github.com/"))
        .or_else(|| remote.strip_prefix("git@github.com:"))
        .or_else(|| remote.strip_prefix("ssh://git@github.com/"))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    (!owner.is_empty() && !repo.is_empty() && parts.next().is_none()).then(|| format!("{owner}/{repo}"))
}

/// Есть ли файл по ссылке (публичный ли репозиторий): HEAD-запрос системным curl.
fn is_public(url: &str) -> bool {
    quiet(if cfg!(windows) { "curl.exe" } else { "curl" })
        .args(["-s", "-o", if cfg!(windows) { "NUL" } else { "/dev/null" }, "-I", "-L", "-w", "%{http_code}", "--max-time", "10", url])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim() == "200")
}

/// Ищет иконку и репозиторий мода на GitHub. Долго (git, сеть) — вызывать в фоне.
/// Иконка берётся, только если файл уже отправлен в публичный репозиторий.
pub fn resolve_links(root: &Path, icon: Option<&Path>) -> ModLinks {
    let mut links = ModLinks::default();
    let Some(slug) = git(root, &["remote", "get-url", "origin"]).and_then(|r| github_slug(&r)) else { return links };
    let repo = format!("https://github.com/{slug}");
    if !is_public(&repo) {
        return links;
    }
    links.repo = Some(repo);
    let (Some(icon), Some(top), Some(branch)) = (icon, git(root, &["rev-parse", "--show-toplevel"]), git(root, &["rev-parse", "--abbrev-ref", "HEAD"])) else {
        return links;
    };
    let abs = root.join(icon);
    let Ok(rel) = abs.strip_prefix(PathBuf::from(top.replace('/', std::path::MAIN_SEPARATOR_STR))) else { return links };
    let rel = rel.to_string_lossy().replace('\\', "/");
    // Файл должен быть в ветке на GitHub, а не только на диске.
    let pushed = git(root, &["ls-tree", "--name-only", &format!("origin/{branch}"), "--", &rel]).is_some();
    let url = format!("https://raw.githubusercontent.com/{slug}/{branch}/{rel}");
    if pushed && is_public(&url) {
        links.icon = Some(url);
    }
    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_remotes() {
        assert_eq!(github_slug("https://github.com/forma-openworks/forma-mycelium.git").as_deref(), Some("forma-openworks/forma-mycelium"));
        assert_eq!(github_slug("git@github.com:holo/ruby-mod.git").as_deref(), Some("holo/ruby-mod"));
        assert_eq!(github_slug("ssh://git@github.com/holo/ruby-mod/").as_deref(), Some("holo/ruby-mod"));
        assert_eq!(github_slug("https://gitlab.com/holo/ruby-mod"), None);
        assert_eq!(github_slug("https://github.com/holo"), None);
    }

    fn project() -> Project {
        let root = std::env::temp_dir().join(format!("mycelium-discord-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut p = Project::open(&root).unwrap();
        p.info.mod_name = Some("Test Mod".into());
        p.info.mod_id = Some("testmod".into());
        p.info.mod_version = Some("1.0.0".into());
        p.info.minecraft = Some("1.21.1".into());
        p.info.loaders = vec![Loader::NeoForge];
        p.info.loader_version = Some("21.1.252".into());
        p
    }

    #[test]
    fn statuses_combine_client_and_server() {
        let p = project();
        let mut input = PresenceInput { project: Some(&p), running: Vec::new(), crashed: None, started_unix: 10, away: false, secret: false, links: None };
        assert_eq!(status(&input), "Status: writing code");
        input.running = vec![(RunKind::Client, "Client".into())];
        assert_eq!(status(&input), "Status: running the client");
        input.running.push((RunKind::Server, "Server".into()));
        assert_eq!(status(&input), "Status: running client & server");
        input.running.clear();
        input.crashed = Some(RunKind::Client);
        assert_eq!(status(&input), "Status: fixing a crash 💥");

        let a = activity(&input);
        assert_eq!(a.details, "Mod: Test Mod (testmod)");
        assert!(a.large_image.ends_with("/mycelium.png"), "без иконки мода — иконка IDE");
        assert_eq!(a.small_text.as_deref(), Some("NeoForge 21.1.252 · Minecraft 1.21.1"));
        assert!(a.small_image.as_deref().is_some_and(|s| s.ends_with("/neoforge.png")));

        let links = ModLinks { icon: Some("https://raw.githubusercontent.com/holo/ruby/main/icon.png".into()), repo: Some("https://github.com/holo/ruby".into()) };
        input.links = Some(&links);
        let a = activity(&input);
        assert_eq!(a.large_image, links.icon.clone().unwrap());
        assert_eq!(a.large_text, "Test Mod 1.0.0");
        assert_eq!(a.buttons.len(), 2);

        // Секретный проект: ни названия, ни иконки, ни ссылки на репозиторий.
        input.secret = true;
        let a = activity(&input);
        assert!(!a.details.contains("Test Mod"));
        assert!(a.large_image.ends_with("/mycelium.png"));
        assert_eq!(a.buttons.len(), 1);
        let json = a.to_json();
        assert_eq!(json["timestamps"]["start"], 10);
        assert!(json["assets"]["small_image"].is_string());
    }
}
