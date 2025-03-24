use crate::highlight;
use json::{object, JsonValue};
use log::{error, info, warn};
use std::collections;
use std::env;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

const BUFFER_SIZE: usize = 100;

pub struct LSPEvent {
    name: String,
    data: JsonValue,
}

pub struct LSPData {
    thread_running: bool,
    request_writer: Option<Box<std::process::ChildStdin>>,
    last_request: usize,
    queue: collections::LinkedList<LSPEvent>,
}

pub type LSP = Arc<Mutex<LSPData>>;

#[derive(Clone)]
pub struct LSPHighlight {
    pub pos: usize,
    pub len: usize,

    pub color: highlight::Color,
}

pub fn to_uri(s: &String) -> String {
    "file://".to_string() + &env::current_dir().unwrap().to_str().unwrap() + &"/".to_string() + s
}

pub fn spawn_lsp(s: LSP, cmd: String) -> std::io::Result<()> {
    {
        let s = s.clone();
        thread::spawn(move || {
            {
                info!(target: "lsp", "Started background thread");
                s.lock().unwrap().thread_running = true;
            }
            lsp_background(s.clone(), cmd);
            {
                info!(target: "lsp","Ended background thread");
                s.lock().unwrap().thread_running = false;
            }
        });
    }

    loop {
        std::thread::sleep(std::time::Duration::from_millis(150));
        let t = s.lock().unwrap();
        if t.request_writer.is_some() || !t.thread_running {
            break;
        }
    }

    let _resp = s.lock().unwrap().run_request(
        "initialize",
        object! {
            capabilities: {}
        },
    )?;

    s.lock()
        .unwrap()
        .run_notification("initialized", object! {})?;

    Ok(())
}

pub fn lsp_background(s: LSP, cmd: String) {
    let mut cmd = Command::new(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    let stdout_reader = cmd.stdout.as_mut().unwrap();
    {
        s.lock().unwrap().request_writer = Some(Box::new(cmd.stdin.unwrap()));
    }

    info!(target:"lsp", "launched lsp server");

    loop {
        let mut line = String::new();
        let mut tmp_buf = [0_u8];

        loop {
            // read up to 10 bytes
            stdout_reader.read(&mut tmp_buf[..]).expect("bad line?");
            line.extend(std::str::from_utf8(&tmp_buf[0..1]).unwrap().chars());

            if tmp_buf[0] == b'\n' {
                break;
            }
        }

        let mut buffer = [0_u8; BUFFER_SIZE];

        let dig = line
            .split("\n")
            .nth(0)
            .unwrap()
            .split(":")
            .last()
            .unwrap()
            .replace("\r", "");

        let mut len: usize = dig[1..].parse().unwrap();
        let mut result = line.split("\n").last().unwrap().to_string();

        stdout_reader.read(&mut buffer[0..2]).unwrap();

        while len > buffer.len() {
            // read up to 10 bytes
            let l = stdout_reader.read(&mut buffer[..]).unwrap();
            len -= l;

            result.extend(std::str::from_utf8(&buffer[..l]).unwrap().chars());
        }

        result = result.replace("\r", "");

        let l = stdout_reader.read(&mut buffer[..len]).unwrap();

        result.extend(std::str::from_utf8(&buffer[..l]).unwrap().chars());

        if !result.ends_with('}') {
            _ = result.pop();
        }

        let json = json::parse(&result).expect("bad json");

        print!("{:?}", json["id"]);

        if let JsonValue::String(name) = &json["method"] {
            let s = &mut s.lock().unwrap();
            s.queue.push_front(LSPEvent {
                name: name.to_string(),
                data: json["params"].clone(),
            })
        } else if let JsonValue::Number(resp_id) = &json["id"] {
            warn!(target: "lsp", "implement callback for request {}", resp_id);
            // s.queue.push_front(LSPEvent {
            //     name: name.to_string(),
            //     data: json["params"].clone(),
            // })
        } else {
            warn!(target: "lsp", "malformed message {}", json);
        }
    }
}

impl LSPData {
    pub fn new() -> Self {
        LSPData {
            thread_running: false,
            request_writer: None,
            last_request: 0,
            queue: collections::LinkedList::new(),
        }
        .into()
    }

    pub fn update(&mut self) {
        if !self.thread_running {
            return;
        };

        for i in &self.queue {
            match &i.name {
                u => warn!(target: "lsp", "unhandled lsp event {}", u),
            }
        }

        self.queue = collections::LinkedList::new();
    }

    pub fn run_notification(&mut self, method: &str, params: JsonValue) -> std::io::Result<()> {
        if let Some(writer) = &mut self.request_writer {
            let content = object! {
                jsonrpc: "2.0",
                method: method,
                params: params,
            }
            .dump();

            writer.write(
                format!("Content-Length: {}\r\n\r\n{}", content.len(), content).as_bytes(),
            )?;
            writer.flush()?;

            info!(target: "lsp", "notification: {}", method);

            Ok(())
        } else {
            Ok(())
        }
    }

    pub fn run_request(
        &mut self,
        method: &str,
        params: JsonValue,
    ) -> std::io::Result<Option<usize>> {
        if let Some(writer) = &mut self.request_writer {
            let request_id;
            request_id = self.last_request;

            self.last_request += 1;

            let content = object! {
                jsonrpc: "2.0",
                id: request_id,
                method: method,
                params: params,
            }
            .dump();

            writer.write(
                format!("Content-Length: {}\r\n\r\n{}", content.len(), content).as_bytes(),
            )?;
            writer.flush()?;

            info!(target: "lsp", "lsp request: {}, id: {}", method, request_id);

            Ok(Some(request_id))
        } else {
            Ok(None)
        }
    }

    pub fn open_file(&mut self, file: String, content: String) -> std::io::Result<()> {
        self.run_notification(
            "textDocument/didOpen",
            object! {
                textDocument: {
                    languageId: "nim",
                    version: 0,
                    uri: to_uri(&file),
                    text: content,
                }
            },
        )?;

        self.run_request(
            "textDocument/semanticTokens/full",
            object! {
                textDocument: {
                    uri: to_uri(&file)
                },
            },
        )?;

        Ok(())
    }

    pub fn save_file(&mut self, file: String, content: String) -> std::io::Result<()> {
        self.run_notification(
            "textDocument/didChange",
            object! {
                textDocument: {
                    version: 1,
                    uri: to_uri(&file)
                },
                contentChanges: [
                    {
                        text: content.clone(),
                    }
                ]
            },
        )?;

        self.run_request(
            "textDocument/semanticTokens/full",
            object! {
                textDocument: {
                    uri: to_uri(&file)
                },
            },
        )?;

        Ok(())
    }

    pub fn close_file(&mut self, file: String) -> std::io::Result<()> {
        self.run_notification(
            "textDocument/didClose",
            object! {
                textDocument: {
                    uri: to_uri(&file)
                },
            },
        )?;

        Ok(())
    }

    pub fn get_highlight(&mut self, _file: String) -> std::io::Result<Vec<Vec<LSPHighlight>>> {
        Ok(vec![])
        //Ok(vec![vec![LSPHighlight {
        //    pos: 0,
        //    len: 10,
        //    color: highlight::Color::Link("function".to_string()),
        //}]])
    }

    // pub fn get_data(&mut self) -> std::io::Result<()> {
    //     let data = Vec::new();
    //     self.stdout_reader.read_to_end(&mut data)?;
    //     println!("data\n{}", data);

    //     Ok(())
    // }
}
