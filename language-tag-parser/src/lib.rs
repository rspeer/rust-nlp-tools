//! A simple implementation of language tag parsing that uses no external
//! data, because we need it to generate that external data.

use std::str::from_utf8;

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

/// LanguageTags are immutable, Sized, and passed by value. Think of them
/// as a big enum, not as a string. Each one takes up 10 bytes.
#[derive(PartialEq, Debug)]
pub struct LanguageTag {
    data: [u8; 10],
}

impl LanguageTag {
    /// This internal function parses a string slice into a LanguageTag,
    /// assuming that the tag has already been normalized into the character
    /// range [-0-9a-z].
    fn parse_raw(tag: &str) -> Result<LanguageTag, LanguageTagError> {
        let mut parts = tag.split("-");
        let mut target: [u8; 10] = [PAD; 10];

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
                write_into_fixed(&mut target, language_ref, 0, 3)
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
        Ok(LanguageTag { data: target })
    }

    pub fn parse(tag: &str) -> Result<LanguageTag, LanguageTagError> {
        let normal_tag: String = tag.replace("_", "-").to_lowercase();
        Ok(LanguageTag::parse_raw(&normal_tag)?)
    }

    pub fn internal_bytes(&self) -> [u8; 10] {
        self.data
    }

    pub fn as_literal(&self) -> String {
        let s = from_utf8(&self.data).unwrap();
        format!("*b\"{}\"", s)
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
        let tag = LanguageTag::parse("zh-hant-tw").unwrap();
        assert_eq!(tag.as_literal(), "*b\"zh HantTW \"");

        let tag = LanguageTag::parse("en").unwrap();
        assert_eq!(tag.as_literal(), "*b\"en        \"");
    }
}
