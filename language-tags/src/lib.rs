#[macro_use]
extern crate lazy_static;
use std::str::{FromStr, from_utf8_unchecked};
pub mod languages;
mod langdata;

// Tag data is padded with spaces
const PAD: u8 = 0x20;

#[derive(PartialEq, Debug)]
pub enum LanguageTagError {
    // The tag contained a character outside of [-0-9A-Za-z_]
    InvalidCharacter,

    // The subtag we're parsing has an unexpected shape, or came in the
    // wrong order
    SubtagFormatError,

    // We can't even parse a subtag from here
    ParseError,
}

#[derive(PartialEq)]
enum ParserState {
    AfterLanguage(i32),
    AfterScript,
    AfterRegion,
    AfterVariant,
}

#[derive(PartialEq, Debug)]
pub struct LanguageTag {
    data: [u8; 10],
}

impl LanguageTag {
    /// Get the 2- or 3-character language code as a String, giving "und" if
    /// the language is unknown.
    pub fn language_code(&self) -> String {
        unsafe { from_utf8_unchecked(&self.data[0..3]).trim_right_matches(' ').to_string() }
    }

    /// Get the 2- or 3-character language code as an Option<String>, giving
    /// None if the language is unknown.
    pub fn get_language(&self) -> Option<String> {
        let code = self.language_code();
        if code == "und" { None } else { Some(code) }
    }

    /// Get the 4-character script code as an Option<String>, giving None
    /// if the script is unset. This returns None in the case of an implicit
    /// script: that is, the script of code `en` is `None`, not `Some("Latn")`.
    pub fn get_script(&self) -> Option<String> {
        unsafe {
            match self.data[3] {
                PAD => None,
                _ => Some(from_utf8_unchecked(&self.data[3..7]).to_string()),
            }
        }
    }

    /// Get the region code as an Option<String>. It will contain a 2-letter
    /// ISO region code or a 3-digit number, or it will be None if the region
    /// is unset.
    pub fn get_region(&self) -> Option<String> {
        unsafe {
            match self.data[7] {
                PAD => None,
                _ => {
                    Some(from_utf8_unchecked(&self.data[7..10]).trim_right_matches(' ').to_string())
                }
            }
        }
    }

    pub fn to_string(&self) -> String {
        let lang: String = self.language_code();
        match self.get_script() {
            Some(script) => {
                match self.get_region() {
                    Some(region) => format!("{}-{}-{}", lang, script, region),
                    None => format!("{}-{}", lang, script),
                }
            }
            None => {
                match self.get_region() {
                    Some(region) => format!("{}-{}", lang, region),
                    None => lang,
                }
            }
        }
    }

    /// This internal function parses a string slice into a 10-byte buffer
    /// that can be turned into a LanguageTag, assuming that the tag has
    /// already been normalized into the character range [-0-9a-z].
    fn parse_into(mut target: &mut [u8; 10], s: &str) -> Result<(), LanguageTagError> {
        let mut parts = s.split("-");

        // Consume the first part, which we know must be a language
        match parts.nth(0) {
            // The value "mis" represents a language tag we can't represent,
            // perhaps because the whole thing is private use, like
            // "x-enochian".
            //
            // TODO: map private-use tags onto the [qaa-qtz] range instead.
            Some("i") | Some("x") => {
                write_into_fixed(&mut target, "mis", 0, 3);
            }
            Some(language_ref) => {
                if !check_characters(language_ref) {
                    return Err(LanguageTagError::InvalidCharacter);
                }
                // Handle replacements for just the language subtag
                match langdata::LANG_REPLACE.get(language_ref) {
                    Some(&repl) => LanguageTag::parse_into(&mut target, &repl).unwrap(),
                    None => write_into_fixed(&mut target, language_ref, 0, 3),
                }
            }
            None => {
                return Err(LanguageTagError::ParseError);
            }
        };
        let mut state: ParserState = ParserState::AfterLanguage(0);
        for subtag_ref in parts {
            let language_state: i32 = {
                match state {
                    ParserState::AfterLanguage(num) => num,
                    _ => -1,
                }
            };
            if !check_characters(subtag_ref) {
                return Err(LanguageTagError::InvalidCharacter);
            }
            if is_extension(subtag_ref) {
                break;
            } else if state != ParserState::AfterVariant && is_variant(subtag_ref) {
                state = ParserState::AfterVariant;
            } else if (language_state >= 0 || state == ParserState::AfterScript) &&
                      is_region(subtag_ref) {
                let mut region_val = subtag_ref.to_uppercase();

                // Handle replacements for the region subtag
                match langdata::REGION_REPLACE.get(&region_val as &str) {
                    Some(&repl) => {
                        region_val = repl.to_uppercase();
                    }
                    None => {}
                }
                write_into_fixed(&mut target, &region_val, 7, 3);
                state = ParserState::AfterRegion;
            } else if language_state >= 0 && is_script(subtag_ref) {
                let (first_letter, rest_letters) = subtag_ref.split_at(1);
                let first_letter_string: String = first_letter.to_uppercase();
                let rest_letters_string: String = rest_letters.to_lowercase();
                let mut script_val = first_letter_string + &rest_letters_string;

                // There is only one script replacement, as of CLDR v30
                if script_val == "Qaai" {
                    script_val = "Zinh".to_string();
                }
                write_into_fixed(&mut target, &script_val, 3, 4);
                state = ParserState::AfterScript;
            } else if language_state >= 0 && language_state < 3 && is_extlang(subtag_ref) {
                // This is an extlang; discard it and just count the fact that
                // it was parsed.
                state = ParserState::AfterLanguage(language_state + 1);
            } else {
                return Err(LanguageTagError::SubtagFormatError);
            }
        }
        Ok(())
    }

    pub fn parse(tag: &str) -> Result<LanguageTag, LanguageTagError> {
        let mut lang_bytes: [u8; 10] = [PAD; 10];
        let normal_tag: String = tag.replace("_", "-").to_lowercase();
        match langdata::LANG_REPLACE.get(&normal_tag as &str) {
            Some(&repl) => {
                LanguageTag::parse_into(&mut lang_bytes, &repl)?;
                Ok(LanguageTag { data: lang_bytes })
            }
            None => {
                LanguageTag::parse_into(&mut lang_bytes, &normal_tag)?;
                Ok(LanguageTag { data: lang_bytes })
            }
        }
    }
}


impl FromStr for LanguageTag {
    type Err = LanguageTagError;

    /// Parse a LanguageTag from its string representation. The result
    /// is a constant-sized Struct that encodes its language, script, and
    /// region.
    fn from_str(s: &str) -> Result<LanguageTag, LanguageTagError> {
        LanguageTag::parse(&s)
    }
}

fn check_characters(subtag: &str) -> bool {
    subtag.bytes().all(|b| (b >= 0x30 && b <= 0x39) || (b >= 0x61 && b <= 0x7a))
}

fn is_extension(subtag: &str) -> bool {
    subtag == "u" || subtag == "x"
}

fn is_variant(subtag: &str) -> bool {
    if subtag.len() == 4 {
        subtag.chars().nth(0).unwrap().is_digit(10)
    } else if subtag.len() >= 5 {
        true
    } else {
        false
    }
}

fn is_region(subtag: &str) -> bool {
    let length = subtag.len();
    match subtag.chars().nth(0) {
        Some(ch) => (ch.is_digit(10) && length == 3) || length == 2,
        None => false,
    }
}

fn is_script(subtag: &str) -> bool {
    subtag.len() == 4
}

fn is_extlang(subtag: &str) -> bool {
    if subtag.len() == 3 {
        !subtag.chars().nth(0).unwrap().is_digit(10)
    } else {
        false
    }
}

fn write_into_fixed(arr: &mut [u8; 10], s: &str, offset: usize, length: usize) {
    for (i, b) in s.bytes().enumerate() {
        if i >= length {
            break;
        }
        arr[offset + i] = b;
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let tag: LanguageTag = "zh-hant-tw".parse().unwrap();
        assert_eq!(tag.language_code(), "zh");
        assert_eq!(tag.get_script(), Some("Hant".to_string()));
        assert_eq!(tag.get_region(), Some("TW".to_string()));
        assert_eq!(tag.to_string(), "zh-Hant-TW".to_string());
    }

    #[test]
    fn test_region() {
        assert!(is_region("gb"));
        assert!(is_region("419"));
    }

    fn parses_as(input: &str, result: &str) {
        let tag: LanguageTag = input.parse().unwrap();
        assert_eq!(tag.to_string(), result.to_string());
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
        parses_as("es-419", "es-419");
        parses_as("en-840", "en-US");
        parses_as("de-DD", "de-DE");
        parses_as("sh-QU", "sr-Latn-EU");
        parses_as("und-Qaai", "und-Zinh");
    }

    #[test]
    fn test_named() {
        let ref tag: LanguageTag = languages::UNKNOWN;
        assert_eq!(tag.language_code(), "und");

        let tag: LanguageTag = "und".parse().unwrap();
        assert_eq!(tag, languages::UNKNOWN);

        let tag: LanguageTag = "zh-hans".parse().unwrap();
        assert_eq!(tag, languages::SIMPLIFIED_CHINESE);

        let tag: LanguageTag = "zh-hant-hk".parse().unwrap();
        assert_eq!(tag, languages::HONG_KONG_CHINESE);
    }
}
