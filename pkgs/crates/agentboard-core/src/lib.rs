use std::collections::BTreeMap;

pub mod model;

pub const STDOUT_LIMIT: usize = 64 * 1024;

pub struct ActionRun {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub message: Option<String>,
}

pub struct RenderedAction {
    pub inputs: BTreeMap<String, String>,
    pub hash: String,
}

pub fn cap(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(STDOUT_LIMIT)]).to_string()
}
