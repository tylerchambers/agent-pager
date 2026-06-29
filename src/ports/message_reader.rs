use crate::AgentPagerError;

pub trait MessageReader {
    fn read_text_stdin(&self) -> Result<String, AgentPagerError>;
    fn read_document_stdin(&self) -> Result<Vec<u8>, AgentPagerError>;
}
