use crate::AgentPagerError;

pub trait SensitiveContentScanner {
    fn scan_text(&self, label: &str, text: &str) -> Result<(), AgentPagerError>;
    fn scan_bytes(&self, label: &str, bytes: &[u8]) -> Result<(), AgentPagerError>;
}
