use std::path::Path;

use crate::AgentPagerError;

pub trait SkillStore {
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, AgentPagerError>;
    fn write_string(&self, path: &Path, contents: &str) -> Result<(), AgentPagerError>;
}
