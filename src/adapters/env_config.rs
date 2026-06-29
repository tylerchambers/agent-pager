use crate::ports::ConfigSource;

#[derive(Debug, Clone, Copy, Default)]
pub struct EnvConfigSource;

impl ConfigSource for EnvConfigSource {
    fn get(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
}
