//! Конфигурации mixin (`railways-common.mixins.json`): пакет, от которого записаны mixin,
//! и на какой стороне каждый из них применяется. Отсюда подсказки знают полное имя
//! mixin из сообщения (`client.MixinFoo`) и что он только клиентский.

use serde_json::Value;

/// Где применяется mixin — по списку конфигурации, в котором он записан.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MixinSide {
    /// Список `mixins`: на клиенте и на сервере.
    Both,
    /// Список `client`: только в клиенте игры.
    Client,
    /// Список `server`: только на выделенном сервере.
    Server,
}

impl MixinSide {
    /// Ключ списка в конфигурации.
    pub fn key(self) -> &'static str {
        match self {
            MixinSide::Both => "mixins",
            MixinSide::Client => "client",
            MixinSide::Server => "server",
        }
    }
}

/// Конфигурация mixin.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MixinConfig {
    /// Имя файла: `railways-common.mixins.json`.
    pub name: String,
    /// Пакет, от которого записаны mixin: `com.railwayteam.railways.mixin`.
    pub package: String,
    /// Список `mixins`.
    pub common: Vec<String>,
    pub client: Vec<String>,
    pub server: Vec<String>,
}

impl MixinConfig {
    /// Разбирает JSON конфигурации. Испорченный JSON — `None`.
    pub fn parse(name: &str, text: &str) -> Option<Self> {
        let v: Value = serde_json::from_str(text).ok()?;
        let list = |key: &str| -> Vec<String> { v.get(key).and_then(Value::as_array).into_iter().flatten().filter_map(|x| x.as_str().map(str::to_owned)).collect() };
        Some(Self {
            name: name.to_owned(),
            package: v.get("package").and_then(Value::as_str).unwrap_or_default().to_owned(),
            common: list("mixins"),
            client: list("client"),
            server: list("server"),
        })
    }

    /// На какой стороне применяется mixin (имя — как в конфигурации: `client.MixinFoo`).
    pub fn side_of(&self, mixin: &str) -> Option<MixinSide> {
        let has = |list: &[String]| list.iter().any(|m| m == mixin);
        if has(&self.client) {
            Some(MixinSide::Client)
        } else if has(&self.server) {
            Some(MixinSide::Server)
        } else if has(&self.common) {
            Some(MixinSide::Both)
        } else {
            None
        }
    }

    /// Полное имя класса mixin: пакет конфигурации и имя из неё.
    pub fn full_name(&self, mixin: &str) -> String {
        if self.package.is_empty() { mixin.to_owned() } else { format!("{}.{mixin}", self.package) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sides_and_full_names() {
        let text = r#"{
            "required": true,
            "package": "com.railwayteam.railways.mixin",
            "compatibilityLevel": "JAVA_17",
            "mixins": ["MixinTrack"],
            "client": ["client.MixinKeyBindsListKeyEntry"],
            "server": ["server.MixinDedicated"]
        }"#;
        let c = MixinConfig::parse("railways-common.mixins.json", text).unwrap();
        assert_eq!(c.side_of("client.MixinKeyBindsListKeyEntry"), Some(MixinSide::Client));
        assert_eq!(c.side_of("MixinTrack"), Some(MixinSide::Both));
        assert_eq!(c.side_of("server.MixinDedicated"), Some(MixinSide::Server));
        assert_eq!(c.side_of("Nope"), None);
        assert_eq!(c.full_name("client.MixinKeyBindsListKeyEntry"), "com.railwayteam.railways.mixin.client.MixinKeyBindsListKeyEntry");
        assert!(MixinConfig::parse("bad.mixins.json", "{ not json").is_none());
    }
}
