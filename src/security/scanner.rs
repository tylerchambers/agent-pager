use crate::{AgentPagerError, ports::SensitiveContentScanner};

use super::SensitiveReason;

#[derive(Debug, Clone, Copy, Default)]
pub struct HeuristicSensitiveContentScanner;

impl HeuristicSensitiveContentScanner {
    fn first_finding(text: &str) -> Option<SensitiveReason> {
        for line in text.lines() {
            if line.contains("-----BEGIN") && line.contains("PRIVATE KEY-----") {
                return Some(SensitiveReason::PrivateKeyMaterial);
            }

            let trimmed = line.trim();
            if has_sensitive_assignment(trimmed) {
                return Some(SensitiveReason::SecretLookingAssignment);
            }

            if contains_token_prefix(trimmed) || contains_aws_access_key(trimmed) {
                return Some(SensitiveReason::TokenLookingValue);
            }

            if contains_telegram_bot_token(trimmed) {
                return Some(SensitiveReason::TelegramBotToken);
            }
        }
        None
    }
}

impl SensitiveContentScanner for HeuristicSensitiveContentScanner {
    fn scan_text(&self, label: &str, text: &str) -> Result<(), AgentPagerError> {
        if let Some(reason) = Self::first_finding(text) {
            return Err(AgentPagerError::SensitivePayload {
                label: label.to_owned(),
                reason,
            });
        }
        Ok(())
    }

    fn scan_bytes(&self, label: &str, bytes: &[u8]) -> Result<(), AgentPagerError> {
        if let Ok(text) = std::str::from_utf8(bytes) {
            self.scan_text(label, text)?;
        }
        Ok(())
    }
}

fn has_sensitive_assignment(line: &str) -> bool {
    let Some((key, value)) = line.split_once('=') else {
        return false;
    };
    if value.trim().is_empty() {
        return false;
    }

    let key = key
        .trim()
        .strip_prefix("export ")
        .unwrap_or_else(|| key.trim())
        .trim()
        .to_ascii_uppercase();

    matches!(
        key.as_str(),
        "TOKEN"
            | "SECRET"
            | "PASSWORD"
            | "PASSWD"
            | "API_KEY"
            | "PRIVATE_KEY"
            | "ACCESS_KEY"
            | "COOKIE"
            | "SESSION"
            | "SESSION_ID"
    ) || key.ends_with("_TOKEN")
        || key.ends_with("_SECRET")
        || key.ends_with("_PASSWORD")
        || key.ends_with("_PASSWD")
        || key.ends_with("_API_KEY")
        || key.ends_with("_PRIVATE_KEY")
        || key.ends_with("_ACCESS_KEY")
        || key.ends_with("_COOKIE")
        || key.ends_with("_SESSION")
        || key.ends_with("_SESSION_ID")
}

fn contains_token_prefix(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.contains("github_pat_")
        || lower.contains("ghp_")
        || lower.contains("gho_")
        || lower.contains("ghu_")
        || lower.contains("ghs_")
        || lower.contains("ghr_")
        || lower.contains("xoxb-")
        || lower.contains("xoxp-")
}

fn contains_aws_access_key(line: &str) -> bool {
    line.split(|character: char| !character.is_ascii_alphanumeric())
        .any(|token| token.len() >= 16 && token.starts_with("AKIA"))
}

fn contains_telegram_bot_token(line: &str) -> bool {
    line.split(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}'
            )
    })
    .any(is_telegram_bot_token)
}

fn is_telegram_bot_token(token: &str) -> bool {
    let Some((bot_id, secret)) = token.split_once(':') else {
        return false;
    };

    bot_id.len() >= 5
        && bot_id.bytes().all(|byte| byte.is_ascii_digit())
        && secret.len() >= 20
        && secret
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
}
