use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref URL_REGEX: Regex = Regex::new(r"https?://\S+").unwrap();
}

pub fn extract_urls(text: &str) -> Vec<String> {
    URL_REGEX
        .find_iter(text)
        .map(|m| m.as_str().to_string())
        .collect()
}
