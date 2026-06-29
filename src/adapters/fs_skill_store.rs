use std::{fs, io, path::Path};

use crate::{AgentPagerError, ports::SkillStore};

#[derive(Debug, Clone, Copy, Default)]
pub struct FsSkillStore;

impl SkillStore for FsSkillStore {
    fn read_to_string(&self, path: &Path) -> Result<Option<String>, AgentPagerError> {
        match fs::read_to_string(path) {
            Ok(contents) => Ok(Some(contents)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(source) => Err(AgentPagerError::SkillRead {
                path: path.to_path_buf(),
                source,
            }),
        }
    }

    fn write_string(&self, path: &Path, contents: &str) -> Result<(), AgentPagerError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| AgentPagerError::SkillCreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(path, contents).map_err(|source| AgentPagerError::SkillWrite {
            path: path.to_path_buf(),
            source,
        })
    }
}
