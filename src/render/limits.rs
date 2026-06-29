#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TelegramLimits {
    pub text_message_chars: usize,
    pub document_caption_chars: usize,
}

impl Default for TelegramLimits {
    fn default() -> Self {
        Self {
            text_message_chars: 4096,
            document_caption_chars: 1024,
        }
    }
}
