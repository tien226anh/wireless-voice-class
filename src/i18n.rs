use eframe::Storage;

const LANGUAGE_KEY: &str = "wireless-pa.language";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Vietnamese,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::English, Self::Vietnamese];

    pub const fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Vietnamese => "vi",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Vietnamese => "Tiếng Việt",
        }
    }

    pub const fn text(self, english: &'static str, vietnamese: &'static str) -> &'static str {
        match self {
            Self::English => english,
            Self::Vietnamese => vietnamese,
        }
    }

    pub fn load(storage: Option<&dyn Storage>) -> Option<Self> {
        match storage?.get_string(LANGUAGE_KEY)?.as_str() {
            "en" => Some(Self::English),
            "vi" => Some(Self::Vietnamese),
            _ => None,
        }
    }

    pub fn save(self, storage: &mut dyn Storage) {
        storage.set_string(LANGUAGE_KEY, self.code().to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemoryStorage(HashMap<String, String>);

    impl Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }
        fn flush(&mut self) {}
    }

    #[test]
    fn first_launch_and_unknown_language_require_a_choice() {
        assert_eq!(Language::load(None), None);
        let mut storage = MemoryStorage::default();
        assert_eq!(Language::load(Some(&storage)), None);
        storage.set_string(LANGUAGE_KEY, "unknown".to_owned());
        assert_eq!(Language::load(Some(&storage)), None);
    }

    #[test]
    fn language_choice_survives_reload_and_can_be_changed() {
        let mut storage = MemoryStorage::default();
        for language in Language::ALL {
            language.save(&mut storage);
            assert_eq!(Language::load(Some(&storage)), Some(language));
        }
    }

    #[test]
    fn translated_text_tracks_the_selected_language() {
        assert_eq!(Language::English.text("Start", "Bắt đầu"), "Start");
        assert_eq!(Language::Vietnamese.text("Start", "Bắt đầu"), "Bắt đầu");
    }
}
