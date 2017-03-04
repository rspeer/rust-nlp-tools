#[macro_use]
extern crate phf;
extern crate language_tag_parser;

use std::str::FromStr;
use std::fmt;
pub use language_tag_parser::{LanguageCodeError, encode_tag, decode_tag, decode_language,
                              decode_extlang, decode_script, decode_region, update_code,
                              language_pair_bytes, LANGUAGE_MASK, LANGUAGE_EXT_MASK, SCRIPT_MASK,
                              REGION_MASK, INHERIT_SCRIPT, INHERIT_SCRIPT_OLD, EMPTY_CODE};
pub mod langdata;
pub mod languages;

const SIMPLIFIED: u64 = languages::SIMPLIFIED_CHINESE.data & SCRIPT_MASK;
const TRADITIONAL: u64 = languages::TRADITIONAL_CHINESE.data & SCRIPT_MASK;

/// A LanguageCode is a wrapper around a 64-bit integer, so don't worry
/// about copying them around. Think of this as a big enum.
#[derive(PartialEq, Debug, Clone, Copy)]
pub struct LanguageCode {
    data: u64,
}

impl fmt::Display for LanguageCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "lang(\"{}\")", self.to_string())
    }
}

impl LanguageCode {
    pub fn new(val: u64) -> LanguageCode {
        LanguageCode { data: val }
    }

    /// Get the 2- or 3-character language subtag as a String, giving "und" if
    /// the language is unknown.
    pub fn language_subtag(self) -> String {
        decode_language(self.data)
    }

    /// Get the 2- or 3-character language code as an Option<String>, giving
    /// None if the language is unknown.
    pub fn get_language(self) -> Option<String> {
        let subtag = self.language_subtag();
        if subtag == "und" { None } else { Some(subtag) }
    }

    /// Get the extlang subtag, if present. For example, Proto-Indo-European
    /// has the tag "ine-pro" with the extlang "pro". For most languages,
    /// this will be `None`.
    pub fn get_extlang(self) -> Option<String> {
        decode_extlang(self.data)
    }

    /// Remove information about script and region from this language code,
    /// leaving a code that only distinguishes the language itself. This can
    /// be useful in a match statement in NLP applications that only need
    /// to distinguish the language. However, you lose the benefit of
    /// language matching -- when languages are nearly the same, such as
    /// `ms` and `id`, you need to match both explicitly.
    pub fn language_only(self) -> LanguageCode {
        LanguageCode { data: self.data & LANGUAGE_EXT_MASK }
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
                        val = update_code(update_code(val, newlang), val & !LANGUAGE_EXT_MASK);
                    }
                    None => {}
                }

                // The only script replacement is Qaai -> Zinh.
                // (I don't even know when you would use this.)
                let script_val: u64 = val & SCRIPT_MASK;
                if script_val == INHERIT_SCRIPT_OLD {
                    val = update_code(val, INHERIT_SCRIPT);
                }

                let region_val: u64 = val & REGION_MASK;
                match langdata::REGION_REPLACE.get(&region_val) {
                    Some(&newregion) => {
                        val = update_code(val, newregion);
                    }
                    None => {}
                }
                Ok(LanguageCode::new(val))
            }
        }
    }

    /// Get a sequence of more general versions of this code.
    pub fn broaden(self) -> Vec<LanguageCode> {
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
    pub fn maximize(self) -> Self {
        if (self.data & LANGUAGE_MASK != 0) && (self.data & SCRIPT_MASK != 0) &&
           (self.data & REGION_MASK != 0) {
            // We can tell this code is already maximal.
            return self;
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
                        return LanguageCode::new(update_code(max, self.data));
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
    pub fn minimize(self) -> Self {
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
        return self;
    }

    /// Get the distance between two maximized language codes,
    /// comparing just the language portion.
    fn match_distance_language(self, other: LanguageCode) -> i32 {
        let lang1: u64 = self.data & LANGUAGE_EXT_MASK;
        let lang2: u64 = other.data & LANGUAGE_EXT_MASK;
        if lang1 == lang2 {
            0
        } else {
            let pair = language_pair_bytes(lang1, lang2);
            match langdata::MATCH_DISTANCE.get(&pair) {
                Some(&dist) => dist,
                None => 80,
            }
        }
    }

    /// Get the distance between two maximized language codes,
    /// disregarding the region (which has already been checked)
    /// and comparing them at the script level.
    fn match_distance_script(self, other: LanguageCode) -> i32 {
        let lang1: u64 = self.data & LANGUAGE_EXT_MASK;
        let lang2: u64 = other.data & LANGUAGE_EXT_MASK;
        let script1: u64 = self.data & SCRIPT_MASK;
        let script2: u64 = other.data & SCRIPT_MASK;
        if (lang1 | script1) == (lang2 | script2) {
            0
        } else if script1 == script2 {
            // When the scripts are the same, go on to matching the language.
            // We can check this first because there's nothing in matching.txt
            // that would give a different result than this, in the case of
            // different languages and the same script.
            self.match_distance_language(other)
        } else {
            let pair = language_pair_bytes(lang1 | script1, lang2 | script2);
            match langdata::MATCH_DISTANCE.get(&pair) {
                Some(&dist) => dist,
                None => {
                    // The one wildcard rule that applies to scripts is about
                    // matching Simplified Chinese vs. Traditional Chinese
                    // characters. It's a bad match, but the Traditional ->
                    // Simplified direction is slightly worse.
                    if script1 == SIMPLIFIED && script2 == TRADITIONAL {
                        15 + self.match_distance_language(other)
                    } else if script1 == TRADITIONAL && script2 == SIMPLIFIED {
                        19 + self.match_distance_language(other)
                    } else {
                        40 + self.match_distance_language(other)
                    }
                }
            }
        }
    }

    /// Get the distance between two maximized language codes, starting
    /// by comparing them at the region level. Either we'll find a known
    /// distance for the language/script/region triples, or we'll
    /// compute a distance for just the region part, and pass the rest
    /// to `match_distance_script`.
    fn match_distance_region(self, other: LanguageCode) -> i32 {
        if self.data == other.data {
            // These codes are the same, so the distance is exactly 0.
            0
        } else {
            // Convert this pair of languages to the form that can be looked
            // up in our pre-computed hashtable, and look it up to see if
            // it's a known distance.
            let pair = language_pair_bytes(self.data, other.data);
            match langdata::MATCH_DISTANCE.get(&pair) {
                Some(&dist) => dist,
                None => {
                    // There's no exact match, so we need to compute a region
                    // distance.
                    let lang1: u64 = self.data & LANGUAGE_EXT_MASK;
                    let lang2: u64 = other.data & LANGUAGE_EXT_MASK;
                    let region1: u64 = self.data & REGION_MASK;
                    let region2: u64 = other.data & REGION_MASK;
                    if region1 == region2 {
                        // If the regions are the same, the region adds 0 distance.
                        // Return just the distance from `match_distance_script()`.
                        self.match_distance_script(other)
                    } else {
                        // There are several wildcard rules that match at the region
                        // level, and the following code implements them (instead of
                        // a system for matching languages on CLDR's wildcard rules,
                        // which would be inefficient).
                        //
                        // After matching a wildcard rule, we still need to add the
                        // distance that comes from the language and script.
                        let lang_region1 = lang1 | region1;
                        let lang_region2 = lang2 | region2;
                        if lang1 == languages::PORTUGUESE.data &&
                           lang2 == languages::PORTUGUESE.data {
                            // The wildcard rules for matching Portuguese imply that
                            // regions of Portuguese match with a distance of 4 only
                            // if they're both "New World" or both "Old World".
                            //
                            // The only kinds of "New World" Portuguese defined by CLDR
                            // are pt-BR and pt-US, and the specific match between those
                            // is given a value of 4 in matching.txt. If one of these
                            // is matched with any other kind of Portuguese, it gets
                            // a distance of 8.
                            if lang_region1 == languages::BRAZILIAN_PORTUGUESE.data ||
                               lang_region2 == languages::BRAZILIAN_PORTUGUESE.data {
                                8 + self.match_distance_script(other)
                            } else if lang_region1 == languages::AMERICAN_PORTUGUESE.data ||
                                      lang_region2 == languages::AMERICAN_PORTUGUESE.data {
                                8 + self.match_distance_script(other)
                            } else {
                                4 + self.match_distance_script(other)
                            }
                        } else if lang1 == languages::ENGLISH.data &&
                                  lang2 == languages::ENGLISH.data {
                            // British English (en-GB) is a close match for many variants
                            // of English in the world, such as en-IN, and these are also a
                            // close match for "International English" (en-001). American
                            // English is farther away from all of these.
                            if lang_region1 == languages::AMERICAN_ENGLISH.data ||
                               lang_region2 == languages::AMERICAN_ENGLISH.data {
                                6 + self.match_distance_script(other)
                            } else if lang_region1 == languages::BRITISH_ENGLISH.data ||
                                      lang_region2 == languages::BRITISH_ENGLISH.data {
                                4 + self.match_distance_script(other)
                            } else if lang_region1 == languages::INTERNATIONAL_ENGLISH.data ||
                                      lang_region2 == languages::INTERNATIONAL_ENGLISH.data {
                                4 + self.match_distance_script(other)
                            } else {
                                5 + self.match_distance_script(other)
                            }
                        } else if lang1 == languages::SPANISH.data &&
                                  lang2 == languages::SPANISH.data {
                            // European Spanish (es-ES) is farther away from other regional
                            // variants of Spanish than they are from each other.
                            // Latin American Spanish (es-419) is a close match for everything
                            // but es-ES.
                            if lang_region1 == languages::EUROPEAN_SPANISH.data ||
                               lang_region2 == languages::EUROPEAN_SPANISH.data {
                                8 + self.match_distance_script(other)
                            } else if lang_region1 == languages::LATIN_AMERICAN_SPANISH.data ||
                                      lang_region2 == languages::LATIN_AMERICAN_SPANISH.data {
                                4 + self.match_distance_script(other)
                            } else {
                                5 + self.match_distance_script(other)
                            }
                        } else {
                            // In languages with no specific wildcard rules, a difference in
                            // region only adds 4 distance.
                            4 + self.match_distance_script(other)
                        }
                    }
                }
            }
        }
    }

    /// Return a number representing the distance between this language
    /// code (the desired language) and another (the supported language).
    ///
    /// A distance of 0 indicates an exact match. Distances up to 10 are
    /// minor variations, and distances up to 20 or 25 should still be
    /// comprehensible, if potentially unsatisfying to the user.
    /// The distance between completely unrelated languages is 124.
    pub fn match_distance(self, other: LanguageCode) -> i32 {
        self.maximize().match_distance_region(other.maximize())
    }

    pub fn find_match(self,
                      rank_penalty: i32,
                      cutoff: i32,
                      possibilities: &Vec<LanguageCode>)
                      -> (LanguageCode, i32) {
        let mut rank_cost: i32 = 0;
        let mut best_match: LanguageCode = languages::UNKNOWN;
        let mut best_distance: i32 = 1000;
        let mut best_cost: i32 = 1000;

        for &other in possibilities {
            let distance: i32 = self.match_distance(other);
            let cost: i32 = distance + rank_cost;
            if distance == 0 {
                return (other, 0);
            }
            if distance < cutoff && cost < best_cost {
                best_match = other;
                best_cost = cost;
                best_distance = distance;
            }
            rank_cost += rank_penalty;
            if rank_cost >= best_cost {
                break;
            }
        }
        (best_match, best_distance)
    }

    pub fn match_desired_with_cutoff(self,
                                     cutoff: i32,
                                     desired: &Vec<LanguageCode>)
                                     -> (LanguageCode, i32) {
        self.find_match(5, cutoff, desired)
    }

    pub fn match_desired(self, desired: &Vec<LanguageCode>) -> (LanguageCode, i32) {
        self.find_match(5, 25, desired)
    }

    pub fn match_supported_with_cutoff(self,
                                       cutoff: i32,
                                       supported: &Vec<LanguageCode>)
                                       -> (LanguageCode, i32) {
        for &other in supported {
            if other == self {
                return (other, 0);
            }
        }
        self.find_match(0, cutoff, supported)
    }

    pub fn match_supported(self, supported: &Vec<LanguageCode>) -> (LanguageCode, i32) {
        self.find_match(0, 25, supported)
    }
}


pub fn match_lists_with_cutoff(rank_penalty: i32,
                               cutoff: i32,
                               desired: &Vec<LanguageCode>,
                               supported: &Vec<LanguageCode>)
                               -> (LanguageCode, i32) {
    let mut rank_cost: i32 = 0;
    let mut best_match: LanguageCode = languages::UNKNOWN;
    let mut best_distance: i32 = 1000;
    let mut best_cost: i32 = 1000;
    for &d in desired {
        let (matched, distance) = d.match_supported_with_cutoff(cutoff, supported);
        let cost: i32 = distance + rank_cost;
        if distance < cutoff && cost < best_cost {
            best_match = d;
            best_cost = cost;
            best_distance = distance;
        }
        rank_cost += rank_penalty;
        if rank_cost >= best_cost {
            break;
        }
    }
    (best_match, best_distance)
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


/// A convenient function for declaring language codes from literals.
/// Parses the given string as a language code, and panics if it does
/// not parse.
pub fn lang(s: &str) -> LanguageCode {
    LanguageCode::parse(&s).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let code: LanguageCode = "zh-hant-tw".parse().unwrap();
        assert_eq!(code.get_language(), Some("zh"));
        assert_eq!(code.get_script(), Some("Hant".to_string()));
        assert_eq!(code.get_region(), Some("TW".to_string()));
        assert_eq!(code.to_string(), "zh-Hant-TW");
    }

    fn parses_as(input: &str, result: &str) {
        let code: LanguageCode = input.parse().unwrap();
        assert_eq!(code.to_string(), result);
    }

    fn maximizes_to(input: &str, result: &str) {
        let code: LanguageCode = input.parse().unwrap();
        assert_eq!(code.maximize().to_string(), result);
    }

    fn minimizes_to(input: &str, result: &str) {
        let code: LanguageCode = input.parse().unwrap();
        assert_eq!(code.minimize().to_string(), result);
    }

    fn check_distance(lang1: &str, lang2: &str, dist: i32) {
        let code1: LanguageCode = lang1.parse().unwrap();
        let code2: LanguageCode = lang2.parse().unwrap();
        assert_eq!(code1.match_distance(code2), dist)
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

        assert_eq!(languages::BRAZILIAN_PORTUGUESE.language_only(),
                   languages::PORTUGUESE);
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

    #[test]
    fn test_distance() {
        check_distance("no", "no", 0);
        check_distance("no", "nb", 0);
        check_distance("en", "en-Latn", 0);
        check_distance("en-US", "en-PR", 4);
        check_distance("en-GB", "en-IN", 4);
        check_distance("en-US", "en-GB", 6);
        check_distance("ta", "en", 14);
        check_distance("mg", "fr", 14);
        check_distance("zh-Hans", "zh-Hant", 19);
        check_distance("zh-Hant", "zh-Hans", 23);
        check_distance("en", "en-Shaw", 46);
        check_distance("en", "ja", 124);
    }

    #[test]
    fn test_distance_named() {
        assert_eq!(languages::NORWEGIAN_BOKMAL.match_distance(languages::NORWEGIAN),
                   1);
        assert_eq!(languages::AMERICAN_ENGLISH.match_distance(languages::BRITISH_ENGLISH),
                   6);
        assert_eq!(languages::CHINESE.match_distance(languages::TRADITIONAL_CHINESE),
                   19);
    }
}
