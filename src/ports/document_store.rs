use crate::{
    AgentPagerError,
    command::DocumentSource,
    domain::{Document, DocumentFileName},
};

pub trait DocumentStore {
    fn read_document(
        &self,
        source: &DocumentSource,
        explicit_name: Option<DocumentFileName>,
    ) -> Result<Document, AgentPagerError>;
}
