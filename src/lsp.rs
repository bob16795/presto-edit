use crate::math::Vector;
use json::object;
use std::env;
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

const BUFFER_SIZE: usize = 100;

pub struct LSP {
    cmd: Child,
}

pub fn to_uri(s: String) -> String {
    "file://".to_string() + &env::current_dir().unwrap().to_str().unwrap() + &"/".to_string() + &s
}

impl LSP {
    pub fn new() -> Self {
        LSP {
            cmd: Command::new(&"nimlsp_debug")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .unwrap(),
        }
    }

    pub fn init(&mut self) -> std::io::Result<()> {
        let stdout = self.cmd.stdout.as_mut().unwrap();
        let stdin = self.cmd.stdin.as_mut().unwrap();
        let mut stdout_reader = BufReader::new(stdout);
        let mut stdin_writer = BufWriter::new(stdin);

        let content = object! {
            jsonrpc: "2.0",
            id: "1",
            method: "initialize",
        }
        .dump();

        stdin_writer
            .write(format!("Content-Length: {}\r\n\r\n{}", content.len(), content).as_bytes())?;
        stdin_writer.flush()?;

        let mut buffer = [0_u8; BUFFER_SIZE];
        let mut line = String::new();

        while !buffer.contains(&b'\n') {
            // read up to 10 bytes
            stdout_reader.read(&mut buffer[..]).unwrap();
            line.extend(std::str::from_utf8(&buffer).unwrap().chars());
        }
        let dig = line
            .split("\n")
            .nth(0)
            .unwrap()
            .split(":")
            .last()
            .unwrap()
            .replace("\r", "");

        let mut len: usize = dig[1..].parse().unwrap();
        let mut result = line
            .split("\n")
            .last()
            .unwrap()
            .to_string()
            .replace("\r", "");

        len -= result.len() - 1;

        while len > buffer.len() {
            // read up to 10 bytes
            let l = stdout_reader.read(&mut buffer[..]).unwrap();
            len -= l;

            result.extend(std::str::from_utf8(&buffer[..l]).unwrap().chars());
        }

        let l = stdout_reader.read(&mut buffer[..len]).unwrap();

        result.extend(std::str::from_utf8(&buffer[..l]).unwrap().chars());

        Ok(())
    }

    pub fn open_file(&mut self, file: String, content: String) -> std::io::Result<()> {
        let stdin = self.cmd.stdin.as_mut().unwrap();
        let mut stdin_writer = BufWriter::new(stdin);

        let content = object! {
            jsonrpc: "2.0",
            method: "textDocument/didOpen",
            params: {
                textDocument: {
                    languageId: "nim",
                    version: 0,
                    uri: to_uri(file),
                    text: content,
                }
            }
        }
        .dump();

        stdin_writer
            .write(format!("Content-Length: {}\r\n\r\n{}", content.len(), content,).as_bytes())?;
        stdin_writer.flush()?;

        Ok(())
    }

    pub fn save_file(&mut self, file: String, content: String) -> std::io::Result<()> {
        let stdin = self.cmd.stdin.as_mut().unwrap();
        let mut stdin_writer = BufWriter::new(stdin);

        let content = object! {
            jsonrpc: "2.0",
            method: "textDocument/didChange",
            params: {
                textDocument: {
                    uri: to_uri(file)
                },
                contentChanges: [
                    {
                        text: content.clone(),
                    }
                ]
            }
        }
        .dump();

        stdin_writer
            .write(format!("Content-Length: {}\r\n\r\n{}", content.len(), content).as_bytes())?;
        stdin_writer.flush()?;

        Ok(())
    }

    pub fn close_file(&mut self, file: String) -> std::io::Result<()> {
        let stdin = self.cmd.stdin.as_mut().unwrap();
        let mut stdin_writer = BufWriter::new(stdin);

        let content = object! {
            jsonrpc: "2.0",
            method: "textDocument/didClose",
            params: {
                textDocument: {
                    uri: to_uri(file),
                }
            }
        }
        .dump();

        stdin_writer
            .write(format!("Content-Length: {}\r\n\r\n{}", content.len(), content,).as_bytes())?;
        stdin_writer.flush()?;

        Ok(())
    }
}
