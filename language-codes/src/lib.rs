#[macro_use]
extern crate phf;
extern crate language_tag_parser;

use std::str::FromStr;
pub use language_tag_parser::{LanguageCodeError, encode_tag, decode_tag, decode_language,
                              decode_extlang, decode_script, decode_region, update_tag,
                              LANGUAGE_MASK, LANGUAGE_EXT_MASK, SCRIPT_MASK, REGION_MASK,
                              INHERIT_SCRIPT, INHERIT_SCRIPT_OLD, EMPTY_CODE};
pub mod langdata;
pub mod languages;

/// A LanguageCode is a wrapper around a 64-bit integer, so don't worry
/// about copying them around. Think of this as a big enum.
#[derive(PartialEq, Debug, Clone, Copy)]
pub struct LanguageCode {
    data: u64,
}

impl LanguageCode {
    pub fn new(val: u64) -> LanguageCode {
        LanguageCode { data: val }
    }

    /// Get the 2- or 3-character language subtag as a String, giving "und" if
    /// the language is unknown.
    pub fn language_subtag(&self) -> String {
        decode_language(self.data)
    }

    /// Get the 2- or 3-character language code as an Option<String>, giving
    /// None if the language is unknown.
    pub fn get_language(&self) -> Option<String> {
        let subtag = self.language_subtag();
        if subtag == "und" { None } else { Some(subtag) }
    }

    pub fn get_extlang(&self) -> Option<String> {
        decode_extlang(self.data)
    }

    /// Get the 4-character script code as an Option<String>, giving None
    /// if the script is unset. This returns None in the case of an implicit
    /// script: that is, the script of code `en` is `None`, not `Some("Latn")`.
    pub fn get_script(&self) -> Option<String> {
        decode_script(self.data)
    }

    /// Get the region code as an Option<String>. It will contain a 2-letter
    /// ISO region code or a 3-digit number, or it will be None if the region
    /// is unset.
    pub fn get_region(&self) -> Option<String> {
        decode_region(self.data)
    }

    pub fn to_string(&self) -> String {
        decode_tag(self.data)
    }

    pub fn parse(tag: &str) -> Result<LanguageCode, LanguageCodeError> {
        let normal_tag: String = tag.replace("_", "-").to_lowercase();
        match langdata::TAG_REPLACE.get(&normal_tag as &str) {
            Some(&repl) => Ok(LanguageCode::new(repl)),
            None => {
                let mut val: u64 = encode_tag(tag)?;
                let lang_val: u64 = val & LANGUAGE_MASK;
                match langdata::LANG_REPLACE.get(&lang_val) {
                    Some(&newlang) => {
                        // We got a new language code for this language, and
                        // need to merge it with what else we know. When both
                        // the old and new tag provide a subtag, keep the new
                        // value for the language subtag, or the old value for
                        // any other subtag.
                        val = update_tag(update_tag(val, newlang), val & !LANGUAGE_EXT_MASK);
                    }
                    None => {}
                }

                // The only script replacement is Qaai -> Zinh.
                // (I don't even know when you would use this.)
                let script_val: u64 = val & SCRIPT_MASK;
                if script_val == INHERIT_SCRIPT_OLD {
                    val = update_tag(val, INHERIT_SCRIPT);
                }

                let region_val: u64 = val & REGION_MASK;
                match langdata::REGION_REPLACE.get(&region_val) {
                    Some(&newregion) => {
                        val = update_tag(val, newregion);
                    }
                    None => {}
                }
                Ok(LanguageCode::new(val))
            }
        }
    }

    /// Get a sequence of more general versions of this code.
    pub fn broaden(&self) -> Vec<LanguageCode> {
        let possibilities = vec![self.data & (LANGUAGE_MASK | SCRIPT_MASK | REGION_MASK),
                                 self.data & (LANGUAGE_MASK | REGION_MASK),
                                 self.data & (LANGUAGE_MASK | SCRIPT_MASK),
                                 self.data & LANGUAGE_MASK,
                                 self.data & REGION_MASK,
                                 self.data & SCRIPT_MASK,
                                 EMPTY_CODE];
        // Skip codes that are equal to the input
        let filtered = possibilities.into_iter().filter(|&n| n != self.data);
        filtered.map(|val| LanguageCode::new(val)).collect()
    }

    /// Get a code with a language, region, and script, filling in the most
    /// likely values based on the values that are specified. For example,
    /// "pt" maximizes to "pt-Latn-BR". This is the "maximize" or "add likely
    /// subtags" operation defined in UTS #35.
    pub fn maximize(&self) -> Self {
        if (self.data & LANGUAGE_MASK != 0) && (self.data & SCRIPT_MASK != 0) &&
           (self.data & REGION_MASK != 0) {
            // We can tell this code is already maximal.
            return self.clone();
        } else {
            match langdata::LIKELY_SUBTAGS.get(&self.data) {
                Some(&max) => {
                    return LanguageCode::new(max);
                }
                None => {}
            }
            for broader_code in self.broaden() {
                match langdata::LIKELY_SUBTAGS.get(&broader_code.data) {
                    Some(&max) => {
                        return LanguageCode::new(update_tag(max, self.data));
                    }
                    None => {}
                }
            }
            panic!("I'm missing data about how to maximize language codes");
        }
    }

    /// Remove any fields that would be added back by `maximize()`. This is
    /// the "remove likely subtags" operation defined in UTS #35.
    ///
    /// We favor scripts over regions -- that is, zh-Hans, not zh-TW. This avoids
    /// returning un-normalized tags (zh-TW is aliased to zh-Hans-TW anyway),
    /// and is more symmetric with `maximize()`.
    pub fn minimize(&self) -> Self {
        let max = self.maximize();
        let possibilities = vec![self.data & LANGUAGE_MASK,
                                 self.data & (LANGUAGE_MASK | SCRIPT_MASK),
                                 self.data & (LANGUAGE_MASK | REGION_MASK)];
        for broader_value in possibilities.into_iter() {
            let code = LanguageCode::new(broader_value);
            if code.maximize() == max {
                return code;
            }
        }
        return self.clone();
    }
}

impl FromStr for LanguageCode {
    type Err = LanguageCodeError;

    /// Parse a LanguageCode from its string representation. The result
    /// is a constant-sized Struct that encodes its language, script, and
    /// region.
    fn from_str(s: &str) -> Result<LanguageCode, LanguageCodeError> {
        LanguageCode::parse(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let code: LanguageCode = "zh-hant-tw".parse().unwrap();
        assert_eq!(code.get_language(), Some("zh".to_string()));
        assert_eq!(code.get_script(), Some("Hant".to_string()));
        assert_eq!(code.get_region(), Some("TW".to_string()));
        assert_eq!(code.to_string(), "zh-Hant-TW".to_string());
    }

    fn parses_as(input: &str, result: &str) {
        let code: LanguageCode = input.parse().unwrap();
        assert_eq!(code.to_string(), result.to_string());
    }

    fn maximizes_to(input: &str, result: &str) {
        let code: LanguageCode = input.parse().unwrap();
        assert_eq!(code.maximize().to_string(), result.to_string());
    }

    fn minimizes_to(input: &str, result: &str) {
        let code: LanguageCode = input.parse().unwrap();
        assert_eq!(code.minimize().to_string(), result.to_string());
    }

    #[test]
    fn test_replacement() {
        parses_as("sh-ME", "sr-Latn-ME");
        parses_as("sh-Cyrl", "sr-Cyrl");
        parses_as("sgn-be-fr", "sfb");
        parses_as("no-bokmal", "nb");
        parses_as("mn-Cyrl-MN", "mn-MN");
        parses_as("zh-CN", "zh-Hans-CN");
        parses_as("i-hak", "hak");
        parses_as("en-UK", "en-GB");
        parses_as("fra", "fr");
        parses_as("fre", "fr");
        parses_as("fi-zZZZ-zZ", "fi");
        parses_as("es-419", "es-419");
        parses_as("en-840", "en-US");
        parses_as("de-DD", "de-DE");
        parses_as("sh-QU", "sr-Latn-EU");
        parses_as("sh-Qaai", "sr-Zinh");
    }

    #[test]
    fn test_named() {
        let ref lcode: LanguageCode = languages::UNKNOWN;
        assert_eq!(lcode.language_subtag(), "und");

        let lcode: LanguageCode = "und".parse().unwrap();
        assert_eq!(lcode, languages::UNKNOWN);

        let lcode: LanguageCode = "eng".parse().unwrap();
        assert_eq!(lcode, languages::ENGLISH);

        let lcode: LanguageCode = "zh-hans".parse().unwrap();
        assert_eq!(lcode, languages::SIMPLIFIED_CHINESE);

        let lcode: LanguageCode = "zh-hant-hk".parse().unwrap();
        assert_eq!(lcode, languages::HONG_KONG_CHINESE);
    }

    #[test]
    fn test_maximize() {
        maximizes_to("en", "en-Latn-US");
        maximizes_to("ja-US", "ja-Jpan-US");
        maximizes_to("und", "en-Latn-US");
        maximizes_to("und-014", "sw-Latn-TZ");
        maximizes_to("und-Vaii", "vai-Vaii-LR");
    }

    #[test]
    fn test_minimize() {
        minimizes_to("en-Latn-US", "en");
        minimizes_to("ja-Jpan", "ja");
        minimizes_to("ja-JP", "ja");
        minimizes_to("zh-Hant-TW", "zh-Hant");
        minimizes_to("en-Shaw-GB", "en-Shaw");
        minimizes_to("vai-Vaii-LR", "vai");
        minimizes_to("pt-Latn-PT", "pt-PT");
        minimizes_to("zh-Latn-US", "zh-Latn-US");
    }
}
