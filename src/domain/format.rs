use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageFormat {
    Plain,
    MarkdownV2,
    Html,
}

impl MessageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::MarkdownV2 => "markdown-v2",
            Self::Html => "html",
        }
    }
}

impl fmt::Display for MessageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
