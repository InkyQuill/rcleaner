//! Application messages. Command names and persisted report fields stay stable.
use std::{collections::BTreeMap, sync::OnceLock};

type Catalog = BTreeMap<String, String>;
static CATALOG: OnceLock<Catalog> = OnceLock::new();

fn language(locale: &str) -> &str {
    match locale.split(['-', '_', '.', '@', ':']).next().unwrap_or("") {
        "ru" => "ru",
        "ko" => "ko",
        _ => "en",
    }
}

fn catalog(locale: &str) -> Catalog {
    let json = match language(&locale.to_ascii_lowercase()) {
        "ru" => include_str!("../locales/ru.json"),
        "ko" => include_str!("../locales/ko.json"),
        _ => include_str!("../locales/en.json"),
    };
    serde_json::from_str(json).expect("bundled translation catalog must be valid")
}

pub fn text(key: &'static str) -> &'static str {
    CATALOG
        .get_or_init(|| {
            let locale = std::env::var("RCLEANER_LANG")
                .ok()
                .filter(|s| !s.is_empty())
                .or_else(sys_locale::get_locale)
                .unwrap_or_else(|| "en".into());
            catalog(&locale)
        })
        .get(key)
        .map(String::as_str)
        .unwrap_or(key)
}

/// Replace numbered placeholders once; inserted paths are never interpreted.
pub fn message(key: &'static str, args: &[&dyn std::fmt::Display]) -> String {
    let template = text(key);
    let mut result = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        result.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(end) = rest.find('}') {
            if let Ok(index) = rest[1..end].parse::<usize>() {
                if let Some(arg) = args.get(index) {
                    result.push_str(&arg.to_string());
                    rest = &rest[end + 1..];
                    continue;
                }
            }
        }
        result.push('{');
        rest = &rest[1..];
    }
    result.push_str(rest);
    result
}

pub fn weekday(day: u32) -> &'static str {
    text(match day {
        0 => "sunday",
        1 => "monday",
        2 => "tuesday",
        3 => "wednesday",
        4 => "thursday",
        5 => "friday",
        6 => "saturday",
        _ => "unknown",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_variants_and_fallback() {
        for locale in ["ru_RU.UTF-8", "ru-RU", "ru"] {
            assert_eq!(language(locale), "ru");
        }
        assert_eq!(language("ko_KR"), "ko");
        for locale in ["C", "POSIX", "fr_FR", ""] {
            assert_eq!(language(locale), "en");
        }
    }
    #[test]
    fn catalogs_have_identical_keys_and_placeholders() {
        let english = catalog("en");
        for locale in ["ru", "ko"] {
            let translated = catalog(locale);
            assert_eq!(
                english.keys().collect::<Vec<_>>(),
                translated.keys().collect::<Vec<_>>()
            );
            for (key, value) in &english {
                for index in 0..10 {
                    let placeholder = format!("{{{index}}}");
                    assert_eq!(
                        value.matches(&placeholder).count(),
                        translated[key].matches(&placeholder).count(),
                        "{locale}: {key}"
                    );
                }
            }
        }
    }
}
