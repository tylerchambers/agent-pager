use crate::AgentPagerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub success: bool,
    pub stdout: Vec<u8>,
}

pub trait ProcessRunner {
    fn output(&self, program: &str, args: &[&str]) -> Result<ProcessOutput, AgentPagerError>;
}
