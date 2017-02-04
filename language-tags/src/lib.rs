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
    pub fn new(language: Option<&str>, script: Option<&str>, region: Option<&str>) -> LanguageTag {
        let mut lang_bytes: [u8; 10] = [PAD; 10];
        if let Some(lang_str) = language {
            write_into_fixed(&mut lang_bytes, lang_str, 0, 3);
        }
        if let Some(script_str) = script {
            write_into_fixed(&mut lang_bytes, script_str, 3, 4)
        }
        if let Some(region_str) = region {
            write_into_fixed(&mut lang_bytes, region_str, 7, 3)
        }
        LanguageTag { data: lang_bytes }
    }

    /// Construct a LanguageTag quickly from a string slice representing
    /// specifically the language, with no other information such as region
    /// or variant.
    ///
    /// `language` must consist of 2 or 3 lowercase ASCII letters.
    pub fn from_language_subtag(language: &str) -> LanguageTag {
        LanguageTag::new(Some(language), None, None)
    }

    /// Construct a LanguageTag that provides no information.
    pub fn empty() -> LanguageTag {
        LanguageTag::new(None, None, None)
    }

    /// Get the 2- or 3-character language code as a String, giving "und" if
    /// the language is unknown.
    pub fn language_code(&self) -> String {
        unsafe {
            match self.data[0] {
                PAD => "und".to_string(),
                _ => from_utf8_unchecked(&self.data[0..3]).trim_right_matches(' ').to_string(),
            }
        }
    }

    /// Get the 2- or 3-character language code as an Option<String>, giving
    /// None if the language is unknown.
    pub fn get_language(&self) -> Option<String> {
        unsafe {
            match self.data[0] {
                PAD => None,
                _ => {
                    Some(from_utf8_unchecked(&self.data[0..3]).trim_right_matches(' ').to_string())
                }
            }
        }
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
            Some("und") => {}
            Some(language_ref) => {
                if check_characters(language_ref) {
                    write_into_fixed(&mut target, language_ref, 0, 3);
                } else {
                    return Err(LanguageTagError::InvalidCharacter);
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
                let region_val = subtag_ref.to_uppercase();
                write_into_fixed(&mut target, &region_val, 7, 3);
                state = ParserState::AfterRegion;
            } else if language_state >= 0 && is_script(subtag_ref) {
                let (first_letter, rest_letters) = subtag_ref.split_at(1);
                let first_letter_string: String = first_letter.to_uppercase();
                let rest_letters_string: String = rest_letters.to_lowercase();
                let script_val = first_letter_string + &rest_letters_string;
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

    fn parse_revision(&self, tag: &str) -> Result<LanguageTag, LanguageTagError> {
        let mut lang_bytes: [u8; 10] = self.data;
        LanguageTag::parse_into(&mut lang_bytes, &tag)?;
        Ok(LanguageTag { data: lang_bytes })
    }

    pub fn parse(tag: &str) -> Result<LanguageTag, LanguageTagError> {
        let mut lang_bytes: [u8; 10] = [PAD; 10];
        let normal_tag: String = tag.replace("_", "-").to_lowercase();
        let slice_tag: &str = &normal_tag;
        match langdata::REPLACEMENTS.get(slice_tag) {
            Some(&repl) => {
                LanguageTag::parse_into(&mut lang_bytes, &repl)?;
                Ok(LanguageTag { data: lang_bytes })
            }
            None => {
                LanguageTag::parse_into(&mut lang_bytes, slice_tag)?;
                let mut result = LanguageTag { data: lang_bytes };
                match result.get_language() {
                    Some(subtag) => {
                        let subtag_slice: &str = &subtag;
                        match langdata::REPLACEMENTS.get(subtag_slice) {
                            Some(&repl) => {
                                LanguageTag::parse_into(&mut lang_bytes, repl).unwrap();
                                result = result.parse_revision(&repl)?;
                            }
                            None => {}
                        }
                    }
                    None => {}
                }
                Ok(result)
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
    match subtag.chars().nth(0) {
        Some(ch) => ch.is_digit(10) || subtag.len() >= 5,
        None => false,
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
    subtag.len() == 3
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
    fn it_works() {
        let tag: LanguageTag = LanguageTag::empty();
        assert_eq!(tag.language_code(), "und");
    }

    #[test]
    fn test_parse() {
        let tag: LanguageTag = "zh-hant-tw".parse().unwrap();
        assert_eq!(tag.language_code(), "zh");
        assert_eq!(tag.get_script(), Some("Hant".to_string()));
        assert_eq!(tag.get_region(), Some("TW".to_string()));
        assert_eq!(tag.to_string(), "zh-Hant-TW".to_string());
    }

    fn parses_as(input: &str, result: &str) {
        let tag: LanguageTag = input.parse().unwrap();
        assert_eq!(tag.to_string(), result.to_string());
    }

    #[test]
    fn test_replacement() {
        parses_as("sh-ME", "sr-Latn-ME");
        parses_as("sh-Cyrl", "sr-Cyrl");
    }

    #[test]
    fn test_named() {
        let tag: LanguageTag = "zh-hans".parse().unwrap();
        assert_eq!(tag, languages::SIMPLIFIED_CHINESE);
    }
}
