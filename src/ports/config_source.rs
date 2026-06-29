pub trait ConfigSource {
    fn get(&self, key: &str) -> Option<String>;
}
