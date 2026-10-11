//! Локальный канал Discord для Rich Presence: `\\.\pipe\discord-ipc-N` в Windows,
//! сокет `discord-ipc-N` в Linux и macOS. Кадр: код операции и длина (по 4 байта,
//! little-endian), потом JSON.

use std::io::{self, Read, Write};

use serde_json::{Value, json};

const HANDSHAKE: u32 = 0;
const FRAME: u32 = 1;
const CLOSE: u32 = 2;
const PING: u32 = 3;
const PONG: u32 = 4;

trait Pipe: Read + Write + Send {}
impl<T: Read + Write + Send> Pipe for T {}

pub struct Ipc {
    pipe: Box<dyn Pipe>,
    nonce: u64,
}

impl Ipc {
    /// Подключается к запущенному Discord и представляется приложением `client_id`.
    pub fn connect(client_id: &str) -> io::Result<Self> {
        let mut last = io::Error::new(io::ErrorKind::NotFound, "Discord is not running");
        for i in 0..10 {
            match open(i) {
                Ok(pipe) => {
                    let mut ipc = Self { pipe, nonce: 0 };
                    match ipc.handshake(client_id) {
                        Ok(()) => return Ok(ipc),
                        Err(e) => last = e,
                    }
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    fn send(&mut self, op: u32, payload: &Value) -> io::Result<()> {
        let body = payload.to_string();
        let mut frame = Vec::with_capacity(8 + body.len());
        frame.extend_from_slice(&op.to_le_bytes());
        frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
        frame.extend_from_slice(body.as_bytes());
        self.pipe.write_all(&frame)?;
        self.pipe.flush()
    }

    fn recv(&mut self) -> io::Result<(u32, Value)> {
        let mut head = [0u8; 8];
        self.pipe.read_exact(&mut head)?;
        let op = u32::from_le_bytes(head[..4].try_into().expect("4 байта"));
        let len = u32::from_le_bytes(head[4..].try_into().expect("4 байта")) as usize;
        let mut body = vec![0; len];
        self.pipe.read_exact(&mut body)?;
        let value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        Ok((op, value))
    }

    fn handshake(&mut self, client_id: &str) -> io::Result<()> {
        self.send(HANDSHAKE, &json!({"v": 1, "client_id": client_id}))?;
        let (op, v) = self.recv()?;
        if op == FRAME && v.get("evt").and_then(Value::as_str) == Some("READY") {
            return Ok(());
        }
        Err(io::Error::other(v.get("message").and_then(Value::as_str).unwrap_or("handshake failed").to_owned()))
    }

    /// Ставит активность (`None` — убирает). Ждёт ответа на свой запрос.
    pub fn set_activity(&mut self, activity: Option<Value>) -> io::Result<Value> {
        self.nonce += 1;
        let nonce = self.nonce.to_string();
        let args = json!({"pid": std::process::id(), "activity": activity});
        self.send(FRAME, &json!({"cmd": "SET_ACTIVITY", "args": args, "nonce": nonce}))?;
        loop {
            let (op, v) = self.recv()?;
            match op {
                PING => self.send(PONG, &v)?,
                CLOSE => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, v.get("message").and_then(Value::as_str).unwrap_or("closed").to_owned())),
                _ if v.get("nonce").and_then(Value::as_str) == Some(nonce.as_str()) => {
                    if v.get("evt").and_then(Value::as_str) == Some("ERROR") {
                        let msg = v.pointer("/data/message").and_then(Value::as_str).unwrap_or("SET_ACTIVITY failed");
                        return Err(io::Error::other(msg.to_owned()));
                    }
                    return Ok(v);
                }
                _ => {}
            }
        }
    }
}

#[cfg(windows)]
fn open(i: u32) -> io::Result<Box<dyn Pipe>> {
    let file = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{i}"))?;
    Ok(Box::new(file))
}

#[cfg(unix)]
fn open(i: u32) -> io::Result<Box<dyn Pipe>> {
    use std::path::PathBuf;
    let mut bases: Vec<PathBuf> = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"].iter().filter_map(|k| std::env::var_os(k)).map(PathBuf::from).collect();
    bases.push(PathBuf::from("/tmp"));
    let mut last = io::Error::new(io::ErrorKind::NotFound, "Discord is not running");
    for base in bases {
        // Discord из Flatpak и Snap кладёт сокет во вложенную папку.
        for sub in ["", "app/com.discordapp.Discord", "snap.discord"] {
            match std::os::unix::net::UnixStream::connect(base.join(sub).join(format!("discord-ipc-{i}"))) {
                Ok(s) => return Ok(Box::new(s)),
                Err(e) => last = e,
            }
        }
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Канал в памяти: читаем заранее записанные ответы, пишем в буфер.
    struct Fake {
        input: Cursor<Vec<u8>>,
        output: Vec<u8>,
    }
    impl Read for Fake {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.input.read(buf)
        }
    }
    impl Write for Fake {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.output.write(buf)
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn frame(op: u32, v: Value) -> Vec<u8> {
        let body = v.to_string();
        let mut out = op.to_le_bytes().to_vec();
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body.as_bytes());
        out
    }

    #[test]
    fn handshake_and_activity_frames() {
        let mut input = frame(FRAME, json!({"cmd": "DISPATCH", "evt": "READY"}));
        input.extend(frame(PING, json!({})));
        input.extend(frame(FRAME, json!({"cmd": "SET_ACTIVITY", "nonce": "1", "data": {}})));
        let fake = Fake { input: Cursor::new(input), output: Vec::new() };
        let mut ipc = Ipc { pipe: Box::new(fake), nonce: 0 };
        ipc.handshake("123").unwrap();
        ipc.set_activity(Some(json!({"details": "Моддинг"}))).unwrap();
    }

    #[test]
    fn errors_are_reported() {
        let input = frame(FRAME, json!({"cmd": "SET_ACTIVITY", "evt": "ERROR", "nonce": "1", "data": {"message": "bad"}}));
        let mut ipc = Ipc { pipe: Box::new(Fake { input: Cursor::new(input), output: Vec::new() }), nonce: 0 };
        assert_eq!(ipc.set_activity(None).unwrap_err().to_string(), "bad");
        let input = frame(CLOSE, json!({"code": 4000, "message": "Invalid Client ID"}));
        let mut ipc = Ipc { pipe: Box::new(Fake { input: Cursor::new(input), output: Vec::new() }), nonce: 0 };
        assert!(ipc.handshake("x").is_err());
    }
}
