//! JSON-RPC языкового сервера поверх stdin/stdout: заголовок `Content-Length` и тело JSON.
//! Чтение и запись идут в своих потоках, интерфейс забирает сообщения без ожидания.

use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use serde_json::{Value, json};

pub struct Rpc {
    child: Child,
    out: Sender<String>,
    inbox: Receiver<Value>,
    next_id: i64,
}

impl Rpc {
    /// `wake` вызывается на каждое пришедшее сообщение — чтобы интерфейс перерисовался сразу.
    pub fn spawn(mut cmd: Command, wake: impl Fn() + Send + 'static) -> std::io::Result<Self> {
        cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = cmd.spawn()?;
        let stdin = child.stdin.take().expect("stdin перенаправлен");
        let stdout = child.stdout.take().expect("stdout перенаправлен");
        let stderr = child.stderr.take().expect("stderr перенаправлен");

        let (out, out_rx) = channel::<String>();
        std::thread::spawn(move || write_loop(stdin, out_rx));
        let (in_tx, inbox) = channel();
        std::thread::spawn(move || read_loop(stdout, in_tx, wake));
        std::thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                log::debug!("lsp stderr: {line}");
            }
        });
        Ok(Self { child, out, inbox, next_id: 0 })
    }

    fn send(&self, msg: Value) {
        let _ = self.out.send(msg.to_string());
    }

    /// Запрос; возвращает его номер, ответ придёт в `poll`.
    pub fn request(&mut self, method: &str, params: Value) -> i64 {
        self.next_id += 1;
        self.send(json!({"jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params}));
        self.next_id
    }

    pub fn notify(&self, method: &str, params: Value) {
        self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    /// Ответ на запрос сервера.
    pub fn respond(&self, id: Value, result: Value) {
        self.send(json!({"jsonrpc": "2.0", "id": id, "result": result}));
    }

    /// Пришедшие сообщения. `None` — сервер закрыл вывод (завершился).
    pub fn poll(&mut self) -> Option<Vec<Value>> {
        let mut out = Vec::new();
        loop {
            match self.inbox.try_recv() {
                Ok(v) => out.push(v),
                Err(TryRecvError::Empty) => return Some(out),
                Err(TryRecvError::Disconnected) => return if out.is_empty() { None } else { Some(out) },
            }
        }
    }

    /// Ждёт ответа на запрос `id` (остальные сообщения отбрасываются). `true` — дождались.
    pub fn wait_response(&mut self, id: i64, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            match self.inbox.recv_timeout(left) {
                Ok(v) if v.get("id").and_then(Value::as_i64) == Some(id) && v.get("method").is_none() => return true,
                Ok(_) => {}
                Err(_) => return false,
            }
        }
        false
    }

    /// Ждёт, пока процесс завершится сам. `true` — завершился.
    pub fn wait_exit(&mut self, timeout: std::time::Duration) -> bool {
        let deadline = std::time::Instant::now() + timeout;
        while std::time::Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        false
    }

    pub fn kill(&mut self) {
        // Уже завершившийся процесс не трогаем: его номер мог достаться другому.
        if matches!(self.child.try_wait(), Ok(None)) {
            crate::run::kill_tree(&mut self.child);
        }
        let _ = self.child.wait();
    }
}

impl Drop for Rpc {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            self.kill();
        }
    }
}

fn write_loop(mut stdin: impl Write, rx: Receiver<String>) {
    for body in rx {
        let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
        if stdin.write_all(frame.as_bytes()).and_then(|_| stdin.flush()).is_err() {
            break;
        }
    }
}

fn read_loop(stdout: impl Read, tx: Sender<Value>, wake: impl Fn()) {
    let mut reader = BufReader::new(stdout);
    loop {
        let Some(len) = read_header(&mut reader) else { return };
        let mut body = vec![0; len];
        if reader.read_exact(&mut body).is_err() {
            return;
        }
        match serde_json::from_slice::<Value>(&body) {
            Ok(v) => {
                if tx.send(v).is_err() {
                    return;
                }
                wake();
            }
            Err(err) => log::warn!("lsp: bad message: {err}"),
        }
    }
}

/// Заголовки до пустой строки; нужен только `Content-Length`.
fn read_header(reader: &mut impl BufRead) -> Option<usize> {
    let mut len = None;
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let l = line.trim_end();
        if l.is_empty() {
            if len.is_some() {
                return len;
            }
            continue;
        }
        if let Some(v) = l.strip_prefix("Content-Length:") {
            len = v.trim().parse().ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_framed_messages() {
        let data = b"Content-Length: 17\r\n\r\n{\"jsonrpc\":\"2.0\"}Content-Length: 2\r\nContent-Type: x\r\n\r\n{}";
        let mut r = BufReader::new(&data[..]);
        assert_eq!(read_header(&mut r), Some(17));
        let mut b = vec![0; 17];
        r.read_exact(&mut b).unwrap();
        assert_eq!(read_header(&mut r), Some(2));
    }
}
