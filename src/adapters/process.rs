use std::process::Command;

use crate::{
    AgentPagerError,
    ports::{ProcessOutput, ProcessRunner},
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemProcessRunner;

impl ProcessRunner for SystemProcessRunner {
    fn output(&self, program: &str, args: &[&str]) -> Result<ProcessOutput, AgentPagerError> {
        let output = Command::new(program)
            .args(args)
            .output()
            .map_err(|source| AgentPagerError::ProcessLaunch {
                program: program.to_owned(),
                source,
            })?;
        Ok(ProcessOutput {
            success: output.status.success(),
            stdout: output.stdout,
        })
    }
}
