use std::{fs, path::Path};

use crate::{
    AgentPagerError,
    command::DocumentSource,
    domain::{Document, DocumentFileName},
    ports::{DocumentStore, MessageReader},
};

const STDIN_DOCUMENT_FILE_NAME: &str = "agent-pager-document.md";

#[derive(Debug, Clone)]
pub struct FsDocumentStore<R> {
    stdin_reader: R,
}

impl<R> FsDocumentStore<R> {
    pub fn new(stdin_reader: R) -> Self {
        Self { stdin_reader }
    }
}

impl<R> DocumentStore for FsDocumentStore<R>
where
    R: MessageReader,
{
    fn read_document(
        &self,
        source: &DocumentSource,
        explicit_name: Option<DocumentFileName>,
    ) -> Result<Document, AgentPagerError> {
        match source {
            DocumentSource::Stdin => {
                let bytes = self.stdin_reader.read_document_stdin()?;
                let file_name = match explicit_name {
                    Some(file_name) => file_name,
                    None => DocumentFileName::new(STDIN_DOCUMENT_FILE_NAME)?,
                };
                Document::new(file_name, bytes)
            }
            DocumentSource::Path(path) => {
                let bytes = fs::read(path).map_err(|source| AgentPagerError::DocumentRead {
                    path: path.clone(),
                    source,
                })?;
                let file_name = match explicit_name {
                    Some(file_name) => file_name,
                    None => path_file_name(path)?,
                };
                Document::new(file_name, bytes)
            }
        }
    }
}

fn path_file_name(path: &Path) -> Result<DocumentFileName, AgentPagerError> {
    let file_name = path
        .file_name()
        .ok_or(AgentPagerError::DocumentFileNameMissing)?
        .to_string_lossy()
        .into_owned();
    DocumentFileName::new(file_name)
}
