use std::io::{self, Read};

use crate::{AgentPagerError, ports::MessageReader};

#[derive(Debug, Clone, Copy, Default)]
pub struct StdioMessageReader;

impl MessageReader for StdioMessageReader {
    fn read_text_stdin(&self) -> Result<String, AgentPagerError> {
        let mut message = String::new();
        io::stdin()
            .read_to_string(&mut message)
            .map_err(AgentPagerError::StdinRead)?;
        Ok(message)
    }

    fn read_document_stdin(&self) -> Result<Vec<u8>, AgentPagerError> {
        let mut bytes = Vec::new();
        io::stdin()
            .read_to_end(&mut bytes)
            .map_err(AgentPagerError::StdinRead)?;
        Ok(bytes)
    }
}
