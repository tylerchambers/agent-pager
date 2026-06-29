use std::fmt;

use crate::AgentPagerError;

#[derive(Clone, PartialEq, Eq)]
pub struct MessageBody(String);

impl MessageBody {
    pub fn new(input: impl Into<String>) -> Result<Self, AgentPagerError> {
        let body = input.into().trim().to_owned();
        if body.is_empty() {
            return Err(AgentPagerError::EmptyMessage);
        }
        Ok(Self(body))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.0.into_bytes()
    }
}

impl fmt::Debug for MessageBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageBody")
            .field("chars", &self.0.chars().count())
            .finish_non_exhaustive()
    }
}
