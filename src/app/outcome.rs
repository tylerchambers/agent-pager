use super::{DoctorReport, InstallSkillOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendOutcome {
    SentText,
    SentDocument,
    SentTextAsDocument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOutcome {
    Sent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppOutcome {
    Send(SendOutcome),
    Doctor(DoctorReport),
    InstallSkill(InstallSkillOutcome),
    Test(TestOutcome),
}

impl AppOutcome {
    pub fn status_lines(&self) -> Vec<String> {
        match self {
            Self::Send(SendOutcome::SentText) => vec!["sent page to Telegram".to_owned()],
            Self::Send(SendOutcome::SentDocument) => vec!["sent document to Telegram".to_owned()],
            Self::Send(SendOutcome::SentTextAsDocument) => {
                vec!["sent page as document to Telegram".to_owned()]
            }
            Self::Doctor(report) => report.lines.clone(),
            Self::InstallSkill(outcome) => outcome.status_lines(),
            Self::Test(TestOutcome::Sent) => vec!["sent test page to Telegram".to_owned()],
        }
    }

    pub fn failure_message(&self) -> Option<&'static str> {
        match self {
            Self::Doctor(report) if !report.ok => {
                Some("agent-pager doctor found configuration errors")
            }
            _ => None,
        }
    }
}
