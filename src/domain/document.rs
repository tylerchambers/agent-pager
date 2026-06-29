use std::fmt;

use crate::AgentPagerError;

#[derive(Clone, PartialEq, Eq)]
pub struct DocumentFileName(String);

impl DocumentFileName {
    pub fn new(input: impl Into<String>) -> Result<Self, AgentPagerError> {
        let file_name = input.into().trim().to_owned();
        if file_name.is_empty() {
            return Err(AgentPagerError::EmptyDocumentFileName);
        }
        Ok(Self(file_name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl fmt::Debug for DocumentFileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DocumentFileName").field(&self.0).finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Document {
    file_name: DocumentFileName,
    bytes: Vec<u8>,
}

impl Document {
    pub fn new(file_name: DocumentFileName, bytes: Vec<u8>) -> Result<Self, AgentPagerError> {
        if bytes.is_empty() {
            return Err(AgentPagerError::EmptyDocument);
        }
        Ok(Self { file_name, bytes })
    }

    pub fn file_name(&self) -> &DocumentFileName {
        &self.file_name
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_parts(self) -> (DocumentFileName, Vec<u8>) {
        (self.file_name, self.bytes)
    }
}

impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Document")
            .field("file_name", &self.file_name)
            .field("bytes_len", &self.bytes.len())
            .finish()
    }
}
