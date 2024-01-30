use crate::buffer;
use crate::drawer;
use crate::highlight;
use crate::lsp;
use crate::script;
use crate::Status;
use std::collections::HashMap;

pub struct Data {
    pub dr: Box<dyn drawer::Drawer>,
    pub bu: Box<buffer::Buffer>,
    pub status: Status,
    pub binds: HashMap<String, script::Command>,
    pub colors: HashMap<String, highlight::Color>,
    pub auto: HashMap<(String, String), String>,
    pub lsp: lsp::LSP,
}
