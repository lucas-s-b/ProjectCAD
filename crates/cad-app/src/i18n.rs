//! UI languages. English text is the key: `tr` returns the current language's text, or the key itself when the
//! language's table (`i18n/<code>.txt`, lines `English => translation`) has none, so anything untranslated shows in
//! English. Widget names for the MCP mode stay English; only the drawn text changes.
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[default]
    English,
    Spanish,
    French,
    German,
    Portuguese,
    Italian,
    Dutch,
    Polish,
    Russian,
    Ukrainian,
    Turkish,
    Chinese,
    Japanese,
    Korean,
}

impl Language {
    pub const ALL: [Language; 14] = [
        Language::English,
        Language::Spanish,
        Language::French,
        Language::German,
        Language::Portuguese,
        Language::Italian,
        Language::Dutch,
        Language::Polish,
        Language::Russian,
        Language::Ukrainian,
        Language::Turkish,
        Language::Chinese,
        Language::Japanese,
        Language::Korean,
    ];

    /// The language's name in itself, as the language picker lists it.
    pub fn native_name(self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Spanish => "Español",
            Language::French => "Français",
            Language::German => "Deutsch",
            Language::Portuguese => "Português",
            Language::Italian => "Italiano",
            Language::Dutch => "Nederlands",
            Language::Polish => "Polski",
            Language::Russian => "Русский",
            Language::Ukrainian => "Українська",
            Language::Turkish => "Türkçe",
            Language::Chinese => "简体中文",
            Language::Japanese => "日本語",
            Language::Korean => "한국어",
        }
    }

    fn source(self) -> &'static str {
        match self {
            Language::English => "",
            Language::Spanish => include_str!("../i18n/es.txt"),
            Language::French => include_str!("../i18n/fr.txt"),
            Language::German => include_str!("../i18n/de.txt"),
            Language::Portuguese => include_str!("../i18n/pt.txt"),
            Language::Italian => include_str!("../i18n/it.txt"),
            Language::Dutch => include_str!("../i18n/nl.txt"),
            Language::Polish => include_str!("../i18n/pl.txt"),
            Language::Russian => include_str!("../i18n/ru.txt"),
            Language::Ukrainian => include_str!("../i18n/uk.txt"),
            Language::Turkish => include_str!("../i18n/tr.txt"),
            Language::Chinese => include_str!("../i18n/zh.txt"),
            Language::Japanese => include_str!("../i18n/ja.txt"),
            Language::Korean => include_str!("../i18n/ko.txt"),
        }
    }

    /// System font files that hold this language's script, tried in order; empty when egui's own fonts cover it
    /// (Latin, Greek and Cyrillic).
    fn font_files(self) -> &'static [&'static str] {
        match self {
            Language::Chinese => &[
                "C:/Windows/Fonts/msyh.ttc",
                "C:/Windows/Fonts/simsun.ttc",
                "/System/Library/Fonts/PingFang.ttc",
                "/System/Library/Fonts/STHeiti Light.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            ],
            Language::Japanese => &[
                "C:/Windows/Fonts/YuGothM.ttc",
                "C:/Windows/Fonts/meiryo.ttc",
                "C:/Windows/Fonts/msgothic.ttc",
                "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            ],
            Language::Korean => &[
                "C:/Windows/Fonts/malgun.ttf",
                "/System/Library/Fonts/AppleSDGothicNeo.ttc",
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/google-noto-cjk/NotoSansCJK-Regular.ttc",
            ],
            _ => &[],
        }
    }
}

/// `Language::ALL` index of the language `tr` uses; global so any widget can translate without the app at hand.
static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(lang: Language) {
    CURRENT.store(lang as u8, Ordering::Relaxed);
}

pub fn current() -> Language {
    Language::ALL[usize::from(CURRENT.load(Ordering::Relaxed))]
}

/// `English => translation` lines; blank lines and lines starting with `#` are skipped.
fn parse(src: &'static str) -> HashMap<&'static str, &'static str> {
    src.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| l.split_once(" => "))
        .map(|(k, v)| (k.trim_end(), v.trim()))
        .collect()
}

fn table(lang: Language) -> &'static HashMap<&'static str, &'static str> {
    static TABLES: [OnceLock<HashMap<&'static str, &'static str>>; 14] = [const { OnceLock::new() }; 14];
    TABLES[lang as usize].get_or_init(|| parse(lang.source()))
}

/// `text` in the current language; `text` itself when there is no translation.
pub fn tr(text: &str) -> &str {
    tr_or(text, text)
}

/// The current language's text for `key`, or `english` when there is none (for long texts with short keys).
pub fn tr_or<'a>(key: &str, english: &'a str) -> &'a str {
    lookup(current(), key, english)
}

fn lookup<'a>(lang: Language, key: &str, english: &'a str) -> &'a str {
    match lang {
        Language::English => english,
        lang => table(lang).get(key).copied().unwrap_or(english),
    }
}

/// Makes `lang` current and gives egui a font for its script: egui's own fonts plus the first system font of
/// `Language::font_files` that exists, as a fallback after them. Returns false when the language needs a system
/// font and none was found (its text then shows as boxes).
pub fn apply(ctx: &egui::Context, lang: Language) -> bool {
    set(lang);
    let mut fonts = egui::FontDefinitions::default();
    let files = lang.font_files();
    let found = files.iter().find_map(|p| std::fs::read(p).ok());
    let ok = files.is_empty() || found.is_some();
    if let Some(bytes) = found {
        fonts.font_data.insert("system-script".into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts.families.entry(family).or_default().push("system-script".into());
        }
    }
    ctx.set_fonts(fonts);
    ok
}

#[cfg(test)]
#[path = "i18n_tests.rs"]
mod tests;
