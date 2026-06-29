mod scanner;

use std::fmt;

pub use scanner::HeuristicSensitiveContentScanner;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SensitiveReason {
    PrivateKeyMaterial,
    SecretLookingAssignment,
    TokenLookingValue,
    TelegramBotToken,
}

impl fmt::Display for SensitiveReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PrivateKeyMaterial => "private key material",
            Self::SecretLookingAssignment => "secret-looking assignment",
            Self::TokenLookingValue => "token-looking value",
            Self::TelegramBotToken => "Telegram bot token",
        })
    }
}
