//! Readable display names for ISO-639
//! language codes and kbd keymap codes.

/// Returns a readable language name from the ISO-639 code, or the code
/// itself if unknown.
pub fn language_name(code: &str) -> String {
    LANGUAGES
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| code.to_string())
}

/// Returns a readable layout name from the kbd code, or the code itself if
/// unknown.
pub fn keymap_name(code: &str) -> String {
    KEYMAPS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| code.to_string())
}

/// True if `code` has a known display name (used to filter the raw keymap
/// list down to ones we can label sensibly, same as the Python
/// `detect_keymaps` filter: `KeymapName(k) != k`).
pub fn has_known_keymap_name(code: &str) -> bool {
    keymap_name(code) != code
}

const LANGUAGES: &[(&str, &str)] = &[
    ("aa", "Afar"),
    ("af", "Afrikaans"),
    ("an", "Aragonese"),
    ("ar", "Arabic"),
    ("ast", "Asturian"),
    ("be", "Belgian"),
    ("bg", "Bulgarian"),
    ("bhb", "Bhili"),
    ("br", "Breton"),
    ("bs", "Bosnian"),
    ("ca", "Catalan"),
    ("cs", "Czech"),
    ("cy", "Welsh"),
    ("da", "Danish"),
    ("de", "German"),
    ("el", "Greek"),
    ("en", "English"),
    ("es", "Spanish"),
    ("et", "Estonian"),
    ("eu", "Basque"),
    ("fi", "Finnish"),
    ("fo", "Faroese"),
    ("fr", "French"),
    ("ga", "Irish"),
    ("gd", "Scottish Gaelic"),
    ("gl", "Galician"),
    ("gv", "Manx"),
    ("he", "Hebrew"),
    ("hr", "Croatian"),
    ("hsb", "Upper Sorbian"),
    ("hu", "Hungarian"),
    ("id", "Indonesian"),
    ("is", "Icelandic"),
    ("it", "Italian"),
    ("iw", "Hebrew"),
    ("ja", "Japanese"),
    ("ka", "Georgian"),
    ("kk", "Kazakh"),
    ("kl", "Kalaallisut"),
    ("ko", "Korean"),
    ("ku", "Kurdish"),
    ("kw", "Cornish"),
    ("lg", "Ganda"),
    ("lt", "Lithuanian"),
    ("lv", "Latvian"),
    ("mg", "Malagasy"),
    ("mi", "Maori"),
    ("mk", "Macedonian"),
    ("ms", "Malay"),
    ("mt", "Maltese"),
    ("nb", "Norwegian Bokm\u{e5}l"),
    ("nl", "Dutch"),
    ("nn", "Norwegian Nynorsk"),
    ("oc", "Occitan"),
    ("om", "Oromo"),
    ("pl", "Polish"),
    ("pt", "Portuguese"),
    ("ro", "Romanian"),
    ("ru", "Russian"),
    ("sk", "Slovak"),
    ("sl", "Slovenian"),
    ("so", "Somali"),
    ("sq", "Albanian"),
    ("st", "Southern Sotho"),
    ("sv", "Swedish"),
    ("tcy", "Tulu"),
    ("tg", "Tajik"),
    ("th", "Thai"),
    ("tl", "Tagalog"),
    ("tr", "Turkish"),
    ("uk", "Ukrainian"),
    ("uz", "Uzbek"),
    ("wa", "Walloon"),
    ("xh", "Xhosa"),
    ("yi", "Yiddish"),
    ("zh", "Chinese"),
    ("zu", "Zulu"),
];

const KEYMAPS: &[(&str, &str)] = &[
    ("us", "English (US)"),
    ("us-acentos", "English (US, Acentos)"),
    ("us1", "English (US, Variant 1)"),
    ("uk", "English (UK)"),
    ("es", "Spanish"),
    ("es-olpc", "Spanish - OLPC"),
    ("fr", "French"),
    ("fr-afnor", "French - AFNOR"),
    ("fr-bepo", "French - B\u{e9}po"),
    ("de", "German"),
    ("de-mobii", "German - Mobii"),
    ("de_alt_UTF-8", "German - Alt UTF-8"),
    ("it", "Italian"),
    ("it-ibm", "Italian - IBM"),
    ("pt", "Portuguese"),
    ("pt-olpc", "Portuguese - OLPC"),
    ("nl", "Dutch"),
    ("nl2", "Dutch Variant 2"),
    ("fi", "Finnish"),
    ("sv-latin1", "Swedish"),
    ("no", "Norwegian"),
    ("dk", "Danish"),
    ("pl", "Polish"),
    ("pl1", "Polish Variant 1"),
    ("pl2", "Polish Variant 2"),
    ("pl3", "Polish Variant 3"),
    ("pl4", "Polish Variant 4"),
    ("cz", "Czech"),
    ("cz-lat2", "Czech - Latin2"),
    ("cz-qwertz", "Czech - QWERTZ"),
    ("cz-us-qwertz", "Czech - US QWERTZ"),
    ("sk-qwerty", "Slovak - QWERTY"),
    ("sk-qwertz", "Slovak - QWERTZ"),
    ("ru", "Russian"),
    ("ru-ms", "Russian - MS"),
    ("ru-yawerty", "Russian - Yawerty"),
    ("ua", "Ukrainian"),
    ("ua-utf", "Ukrainian - UTF"),
    ("ua-utf-ws", "Ukrainian - UTF with WS"),
    ("dvorak", "Dvorak"),
    ("dvorak-es", "Dvorak - Spanish"),
    ("dvorak-de", "Dvorak - German"),
    ("dvorak-fr", "Dvorak - French"),
    ("dvorak-ca-fr", "Dvorak - Canadian French"),
    ("colemak", "Colemak"),
    ("neo", "Neo"),
    ("et", "Estonian"),
    ("et-nodeadkeys", "Estonian - Nodeadkeys"),
    ("hr", "Croatian"),
    ("bg_bds-utf8", "Bulgarian BDS - UTF8"),
    ("bg_pho-utf8", "Bulgarian Phonetic - UTF8"),
    ("sr-latin", "Serbian - Latin"),
    ("sr-cy", "Serbian - Cyrillic"),
    ("jp", "Japanese (standard)"),
    ("jp106", "Japanese (106)"),
    ("jp-OADG109A", "Japanese (OADG 109A)"),
    ("ko", "Korean (Hangul)"),
    ("kr", "Korean (Hangul)"),
    ("kr106", "Korean (Hangul, 106)"),
    ("cn", "Chinese"),
    ("cn-shift", "Chinese (Shift)"),
    ("tw", "Chinese (Taiwan)"),
    ("th", "Thai"),
    ("th_pat", "Thai (Pattachote)"),
    ("th_tis", "Thai (TIS-820.2538)"),
    ("vn", "Vietnamese"),
    ("ara", "Arabic"),
    ("il", "Hebrew"),
    ("ir", "Persian"),
    ("in", "Indian"),
    ("bd", "Bangla"),
    ("pk", "Urdu"),
    ("mn", "Mongolian"),
    ("kz", "Kazakh"),
    ("latam", "Spanish (Latin America)"),
    ("es-winkeys", "Spanish (Windows)"),
    ("latam-winkeys", "Spanish (Latin America, Windows)"),
    ("jp-winkeys", "Japanese (Windows)"),
    ("kr-hangul", "Korean (Hangul)"),
    ("kr-winkeys", "Korean (Hangul, Windows)"),
    ("us-winkeys", "English (US, Windows)"),
    ("uk-winkeys", "English (UK, Windows)"),
    ("de-winkeys", "German (Windows)"),
    ("fr-winkeys", "French (Windows)"),
    ("it-winkeys", "Italian (Windows)"),
    ("pt-winkeys", "Portuguese (Windows)"),
    ("tr-winkeys", "Turkish (Windows)"),
    ("tr", "Turkish (normal)"),
    ("trf", "Turkish (F)"),
    ("trq", "Turkish (Q)"),
    ("tralt", "Turkish (Alt)"),
    ("tr_f-latin5", "Turkish (F, Latin-5)"),
    ("tr_q-latin5", "Turkish (Q, Latin-5)"),
    ("tr_f", "Turkish (F)"),
    ("tr_q", "Turkish (Q)"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_language_resolves() {
        assert_eq!(language_name("en"), "English");
        assert_eq!(language_name("de"), "German");
    }

    #[test]
    fn unknown_language_falls_back_to_code() {
        assert_eq!(language_name("xx-unknown"), "xx-unknown");
    }

    #[test]
    fn known_keymap_resolves() {
        assert_eq!(keymap_name("us"), "English (US)");
        assert_eq!(keymap_name("fr-bepo"), "French - B\u{e9}po");
    }

    #[test]
    fn unknown_keymap_falls_back_to_code() {
        assert_eq!(keymap_name("totally-unknown-layout"), "totally-unknown-layout");
    }

    #[test]
    fn has_known_keymap_name_filters_unknowns() {
        assert!(has_known_keymap_name("us"));
        assert!(!has_known_keymap_name("totally-unknown-layout"));
    }
}
