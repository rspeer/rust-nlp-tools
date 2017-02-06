pub const LANGUAGE_MASK: u64 = 0x7fff_0000_0000_0000_u64;
pub const PROTO_MASK: u64 = 0x0000_8000_0000_0000_u64;
pub const EXTLANG_MASK: u64 = 0x0000_7fff_0000_0000_u64;
pub const LANGUAGE_EXT_MASK: u64 = 0x7fff_ffff_0000_0000_u64;
pub const SCRIPT_MASK: u64 = 0x0000_0000_7fff_f800_u64;
pub const REGION_MASK: u64 = 0x0000_0000_0000_07ff_u64;
pub const SCRIPT_SHIFT: u64 = 11u64;
pub const EXTLANG_SHIFT: u64 = 32u64;
pub const LANGUAGE_SHIFT: u64 = 48u64;
pub const EMPTY_CODE: u64 = 0u64;
pub const MISSING_CODE: u64 = 1916703853911212032u64;

/// There are three ranges of values a subtag could be
/// encoded as:
///
/// * 0: the subtag is undetermined or unspecified (and therefore isn't being
///   passed to this function)
/// * 1-999: the subtag is a 3-digit number (used for region codes)
/// * 1000 or more: the subtag is made of letters, which will be encoded with
///   five bits each
fn decode_subtag(val: u64) -> Option<String> {
    if val == 0 {
        None
    } else if val < 1000 {
        Some(format!("{number:>0width$}", number = val, width = 3))
    } else {
        let mut chars: Vec<char> = Vec::with_capacity(4);
        let mut remain = val - 1000;
        while remain > 0 {
            let charnum: u64 = remain % 32;
            if charnum > 0 {
                let ch = (96u64 + charnum) as u8 as char;
                chars.push(ch);
            }
            remain >>= 5;
        }
        chars.reverse();
        Some(chars.into_iter().collect::<String>())
    }
}

/// Encode a subtag using the scheme described for `decode_subtag`.
/// This does not take an Option -- you should encode None separately.
/// It does take a length to pad alphabetic subtags to, so that,
/// for example, "enm" sorts before "es".
fn encode_subtag(subtag: &str, length: usize) -> u64 {
    match subtag.parse::<u64>() {
        Ok(val) => val,
        _ => {
            let mut val: u64 = 0;
            for ch in subtag.chars() {
                val <<= 5;
                val += ((ch as u8) - 96u8) as u64;
            }
            val <<= 5 * (length - subtag.len());
            val + 1000
        }
    }
}


#[derive(PartialEq, Debug)]
pub enum LanguageCodeError {
    // The tag contained a character outside of [-0-9A-Za-z_]
    InvalidCharacter(String),

    // The subtag we're parsing has an unexpected shape, or came in the
    // wrong order
    SubtagFormatError(String),

    // We can't even parse a subtag from here
    ParseError(String),
}

#[derive(PartialEq)]
enum ParserState {
    AfterLanguage(i32),
    AfterScript,
    AfterRegion,
    AfterVariant,
}


fn parse_lowercase_tag(tag: &str) -> Result<u64, LanguageCodeError> {
    let mut parts = tag.split("-");
    let mut val: u64 = 0;

    match parts.nth(0) {
        Some("i") | Some("x") => {
            return Ok(MISSING_CODE);
        }
        Some("und") => {}
        Some(language_ref) => {
            if !check_characters(language_ref) {
                return Err(LanguageCodeError::InvalidCharacter(tag.to_string()));
            }
            val |= encode_subtag(language_ref, 3) << LANGUAGE_SHIFT;
        }
        None => {
            return Err(LanguageCodeError::ParseError(tag.to_string()));
        }
    }
    let mut state: ParserState = ParserState::AfterLanguage(0);
    for subtag_ref in parts {
        let language_state: i32 = {
            match state {
                ParserState::AfterLanguage(num) => num,
                _ => -1,
            }
        };
        if !check_characters(subtag_ref) {
            return Err(LanguageCodeError::InvalidCharacter(tag.to_string()));
        }
        if is_extension(subtag_ref) {
            break;
        } else if state != ParserState::AfterVariant && is_variant(subtag_ref) {
            state = ParserState::AfterVariant;
        } else if (language_state >= 0 || state == ParserState::AfterScript) &&
                  is_region(subtag_ref) {
            val |= encode_subtag(subtag_ref, 2);
            state = ParserState::AfterRegion;
        } else if language_state >= 0 && is_script(subtag_ref) {
            val |= encode_subtag(subtag_ref, 4) << SCRIPT_SHIFT;
            state = ParserState::AfterScript;
        } else if language_state >= 0 && language_state < 3 && is_extlang(subtag_ref) {
            // This is an extlang; discard it and just count the fact that
            // it was parsed.
            if subtag_ref == "pro" {
                // This is the most common legitimately-used extlang,
                // indicating a protolanguage. We encode it in one bit.
                val |= PROTO_MASK;
            } else if val & EXTLANG_MASK == 0 {
                // Keep the first non-proto extlang.
                val |= encode_subtag(subtag_ref, 3) << EXTLANG_SHIFT;
            }
            state = ParserState::AfterLanguage(language_state + 1);
        } else {
            return Err(LanguageCodeError::SubtagFormatError(tag.to_string()));
        }
    }
    Ok(val)
}

pub fn parse_tag(tag: &str) -> Result<u64, LanguageCodeError> {
    let normal_tag: String = tag.replace("_", "-").to_lowercase();
    Ok(parse_lowercase_tag(&normal_tag)?)
}

pub fn decode_language(val: u64) -> String {
    match decode_subtag((val & LANGUAGE_MASK) >> LANGUAGE_SHIFT) {
        Some(lang) => lang,
        None => "und".to_string(),
    }
}

pub fn decode_extlang(val: u64) -> Option<String> {
    let proto: bool = val & PROTO_MASK != 0;
    match decode_subtag((val & EXTLANG_MASK) >> EXTLANG_SHIFT) {
        Some(lang) => {
            if proto {
                Some(format!("{}-pro", lang))
            } else {
                Some(lang)
            }
        }
        None => if proto { Some("pro".to_string()) } else { None },
    }
}

pub fn decode_script(val: u64) -> Option<String> {
    match decode_subtag((val & SCRIPT_MASK) >> SCRIPT_SHIFT) {
        Some(script) => {
            let (first_letter, rest_letters) = script.split_at(1);
            let cap_script: String = first_letter.to_uppercase() + &rest_letters;
            Some(cap_script)
        }
        None => None,
    }
}

pub fn decode_region(val: u64) -> Option<String> {
    match decode_subtag(val & REGION_MASK) {
        Some(region) => Some(region.to_uppercase()),
        None => None,
    }
}

pub fn unparse_tag(val: u64) -> String {
    let mut parts: Vec<String> = Vec::with_capacity(4);
    parts.push(decode_language(val));
    match decode_extlang(val) {
        Some(extlang) => parts.push(extlang),
        None => {}
    }
    match decode_script(val) {
        Some(script) => parts.push(script),
        None => {}
    }
    match decode_region(val) {
        Some(region) => parts.push(region),
        None => {}
    }
    parts.join("-")
}

pub fn update_tag(old_tag: u64, new_tag: u64) -> u64 {
    let mut update_mask: u64 = 0;
    if new_tag & LANGUAGE_EXT_MASK != 0 {
        update_mask |= LANGUAGE_EXT_MASK;
    }
    if new_tag & SCRIPT_MASK != 0 {
        update_mask |= SCRIPT_MASK;
    }
    if new_tag & REGION_MASK != 0 {
        update_mask |= REGION_MASK;
    }
    (old_tag & !update_mask) | (new_tag & update_mask)
}

pub fn broader_tags(tag: u64) -> Vec<u64> {
    let possibilities = vec![tag & (LANGUAGE_MASK | SCRIPT_MASK | REGION_MASK),
                             tag & (LANGUAGE_MASK | REGION_MASK),
                             tag & (LANGUAGE_MASK | SCRIPT_MASK),
                             tag & LANGUAGE_MASK,
                             tag & REGION_MASK,
                             tag & SCRIPT_MASK];
    possibilities.into_iter().filter(|&n| n != tag).collect()
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


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subtag() {
        assert_eq!(encode_subtag("999", 3), 999);
        assert_eq!(encode_subtag("aa", 3), 2056);
    }

    fn round_trip(tag: &str) {
        let val = parse_tag(tag).unwrap();
        let decoded = unparse_tag(val);
        assert_eq!(tag, &decoded)
    }

    #[test]
    fn test_parse() {
        round_trip("zh-Hant-TW");
        round_trip("en");
        round_trip("und");
        round_trip("pt-BR");
        round_trip("und-Vaii");
        round_trip("es-419");
        round_trip("ine-pro");
        round_trip("roa-opt-pro");
    }
}
