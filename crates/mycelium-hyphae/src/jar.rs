//! Чтение jar: список файлов, содержимое, манифест, пакеты классов и имя модуля JPMS.
//! jar бывает на диске или вложенным (jar-in-jar) — тогда он читается из памяти.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::Path;

use zip::ZipArchive;

/// Файлы больше этого из jar не читаем (защита от поломанных и огромных архивов).
const MAX_ENTRY: u64 = 64 * 1024 * 1024;

trait ReadSeek: Read + Seek + Send {}
impl<T: Read + Seek + Send> ReadSeek for T {}

/// Открытый jar.
pub struct Jar {
    archive: ZipArchive<Box<dyn ReadSeek>>,
    /// Файлы (без папок) по алфавиту.
    entries: Vec<String>,
}

impl Jar {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file: Box<dyn ReadSeek> = Box::new(BufReader::new(File::open(path)?));
        Self::new(file)
    }

    /// Вложенный jar, прочитанный в память.
    pub fn from_bytes(bytes: Vec<u8>) -> std::io::Result<Self> {
        Self::new(Box::new(Cursor::new(bytes)))
    }

    fn new(reader: Box<dyn ReadSeek>) -> std::io::Result<Self> {
        let archive = ZipArchive::new(reader).map_err(std::io::Error::other)?;
        let mut entries: Vec<String> = archive.file_names().filter_map(Result::ok).filter(|n| !n.ends_with('/')).map(|n| n.into_owned()).collect();
        entries.sort();
        Ok(Self { archive, entries })
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn has(&self, name: &str) -> bool {
        self.entries.binary_search_by(|e| e.as_str().cmp(name)).is_ok()
    }

    /// Есть ли файл, путь которого начинается с `prefix`.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        let i = self.entries.partition_point(|e| e.as_str() < prefix);
        self.entries.get(i).is_some_and(|e| e.starts_with(prefix))
    }

    pub fn read(&mut self, name: &str) -> std::io::Result<Vec<u8>> {
        let mut file = self.archive.by_name(name).map_err(std::io::Error::other)?;
        if file.size() > MAX_ENTRY {
            return Err(std::io::Error::other(format!("{name} is too large")));
        }
        let mut buf = Vec::with_capacity(file.size() as usize);
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Текстовый файл (UTF-8, с BOM или без). Нет файла или он не текст — `None`.
    pub fn read_text(&mut self, name: &str) -> Option<String> {
        if !self.has(name) {
            return None;
        }
        let bytes = self.read(name).ok()?;
        let text = String::from_utf8(bytes).ok()?;
        Some(text.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(text))
    }

    /// Главный раздел `META-INF/MANIFEST.MF`.
    pub fn manifest(&mut self) -> HashMap<String, String> {
        self.read_text("META-INF/MANIFEST.MF").map(|t| parse_manifest(&t)).unwrap_or_default()
    }

    /// Пакеты с классами через точку: `com.simibubi.create.content`. Служебные папки
    /// `META-INF` (в том числе классы для других версий Java) не входят.
    pub fn packages(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .entries
            .iter()
            .filter(|e| e.ends_with(".class") && !e.starts_with("META-INF/"))
            .filter_map(|e| e.rsplit_once('/').map(|(dir, _)| dir.replace('/', ".")))
            .collect();
        out.dedup();
        out.sort();
        out.dedup();
        out
    }

    /// Имя модуля JPMS: из `module-info.class`, иначе `Automatic-Module-Name` манифеста.
    pub fn module_name(&mut self) -> Option<String> {
        let info = std::iter::once("module-info.class".to_owned())
            .chain(self.entries.iter().filter(|e| e.starts_with("META-INF/versions/") && e.ends_with("/module-info.class")).cloned())
            .find(|e| self.has(e));
        if let Some(name) = info.and_then(|e| self.read(&e).ok()).and_then(|bytes| module_info_name(&bytes)) {
            return Some(name);
        }
        self.manifest().remove("Automatic-Module-Name").filter(|n| !n.is_empty())
    }
}

/// Главный раздел манифеста: `Ключ: значение`, продолжение строки начинается с пробела.
pub fn parse_manifest(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut last: Option<String> = None;
    for line in text.lines() {
        if line.is_empty() {
            // Пустая строка заканчивает главный раздел.
            break;
        }
        if let Some(rest) = line.strip_prefix(' ') {
            if let Some(v) = last.as_ref().and_then(|k| out.get_mut(k)) {
                let v: &mut String = v;
                v.push_str(rest);
            }
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_owned();
            out.insert(key.clone(), v.trim().to_owned());
            last = Some(key);
        }
    }
    out
}

/// Имя модуля из `module-info.class`: атрибут `Module` → константа модуля → строка.
pub fn module_info_name(bytes: &[u8]) -> Option<String> {
    let mut r = ClassReader { bytes, at: 0 };
    if r.u32()? != 0xCAFE_BABE {
        return None;
    }
    r.skip(4)?;
    let count = r.u16()? as usize;
    let mut utf8: HashMap<usize, String> = HashMap::new();
    // Константы модулей: индекс → индекс имени.
    let mut modules: HashMap<usize, usize> = HashMap::new();
    let mut i = 1;
    while i < count {
        let tag = r.u8()?;
        match tag {
            1 => {
                let len = r.u16()? as usize;
                let s = r.take(len)?;
                // Модифицированный UTF-8 Java: для имён модулей совпадает с обычным.
                utf8.insert(i, String::from_utf8_lossy(s).into_owned());
            }
            3 | 4 => r.skip(4)?,
            5 | 6 => {
                r.skip(8)?;
                i += 1;
            }
            7 | 8 | 16 | 20 => r.skip(2)?,
            19 => {
                let name = r.u16()? as usize;
                modules.insert(i, name);
            }
            9 | 10 | 11 | 12 | 17 | 18 => r.skip(4)?,
            15 => r.skip(3)?,
            _ => return None,
        }
        i += 1;
    }
    r.skip(6)?; // access_flags, this_class, super_class
    let interfaces = r.u16()? as usize;
    r.skip(interfaces * 2)?;
    for _ in 0..2 {
        // Полей и методов у module-info нет, но формат их допускает.
        let n = r.u16()?;
        for _ in 0..n {
            r.skip(6)?;
            let attrs = r.u16()?;
            for _ in 0..attrs {
                r.skip(2)?;
                let len = r.u32()? as usize;
                r.skip(len)?;
            }
        }
    }
    let attrs = r.u16()?;
    for _ in 0..attrs {
        let name = r.u16()? as usize;
        let len = r.u32()? as usize;
        let body = r.take(len)?;
        if utf8.get(&name).map(String::as_str) == Some("Module") && body.len() >= 2 {
            let module = u16::from_be_bytes([body[0], body[1]]) as usize;
            return modules.get(&module).and_then(|n| utf8.get(n)).cloned();
        }
    }
    None
}

struct ClassReader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> ClassReader<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let s = self.bytes.get(self.at..self.at.checked_add(n)?)?;
        self.at += n;
        Some(s)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.take(n).map(|_| ())
    }
    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }
    fn u16(&mut self) -> Option<u16> {
        self.take(2).map(|b| u16::from_be_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Option<u32> {
        self.take(4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }
}

/// Имя автоматического модуля по имени файла, как его выводит JPMS: без версии,
/// всё, кроме букв и цифр, — точки (`netty-buffer-4.1.97.Final.jar` → `netty.buffer`).
pub fn automatic_module_name(file_name: &str) -> String {
    let stem = file_name.strip_suffix(".jar").unwrap_or(file_name);
    // Версия начинается с «-цифры».
    let bytes = stem.as_bytes();
    let cut = (0..bytes.len().saturating_sub(1)).find(|&i| bytes[i] == b'-' && bytes[i + 1].is_ascii_digit()).unwrap_or(stem.len());
    let mut name: String = stem[..cut].chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '.' }).collect();
    while name.contains("..") {
        name = name.replace("..", ".");
    }
    name.trim_matches('.').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_with_continuations() {
        let m = parse_manifest("Manifest-Version: 1.0\r\nAutomatic-Module-Name: com.example.lo\r\n ng.name\r\nImplementation-Version: 1.2\r\n\r\nName: x\r\nFoo: bar\r\n");
        assert_eq!(m.get("Automatic-Module-Name").map(String::as_str), Some("com.example.long.name"));
        assert_eq!(m.get("Implementation-Version").map(String::as_str), Some("1.2"));
        assert!(!m.contains_key("Foo"), "только главный раздел");
    }

    #[test]
    fn automatic_names() {
        assert_eq!(automatic_module_name("netty-buffer-4.1.97.Final.jar"), "netty.buffer");
        assert_eq!(automatic_module_name("gson-2.10.1.jar"), "gson");
        assert_eq!(automatic_module_name("jsr305-3.0.2.jar"), "jsr305");
        assert_eq!(automatic_module_name("lwjgl_glfw.jar"), "lwjgl.glfw");
    }

    #[test]
    fn module_info_of_a_small_class_file() {
        // module com.example.mod { requires java.base; } — собрано вручную.
        let mut b: Vec<u8> = vec![0xCA, 0xFE, 0xBA, 0xBE, 0, 0, 0, 53];
        let utf = |s: &str| {
            let mut v = vec![1, 0, s.len() as u8];
            v.extend_from_slice(s.as_bytes());
            v
        };
        // Константы: 1 "module-info", 2 Class(1), 3 "com.example.mod", 4 Module(3),
        // 5 "java.base", 6 Module(5), 7 "Module".
        b.extend_from_slice(&[0, 8]);
        b.extend(utf("module-info"));
        b.extend_from_slice(&[7, 0, 1]);
        b.extend(utf("com.example.mod"));
        b.extend_from_slice(&[19, 0, 3]);
        b.extend(utf("java.base"));
        b.extend_from_slice(&[19, 0, 5]);
        b.extend(utf("Module"));
        b.extend_from_slice(&[0x80, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0]); // флаги, this, super, интерфейсы, поля, методы
        b.extend_from_slice(&[0, 1, 0, 7, 0, 0, 0, 6, 0, 4, 0, 0, 0, 0]); // атрибут Module: имя = константа 4
        assert_eq!(module_info_name(&b).as_deref(), Some("com.example.mod"));
        assert_eq!(module_info_name(b"not a class"), None);
    }
}
