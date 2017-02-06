extern crate phf_codegen;
extern crate language_tag_parser;
extern crate json;

use std::env;
use std::path::Path;
use std::io::prelude::*;
use std::io::{BufWriter, BufReader, Error};
use std::fs::File;
use language_tag_parser::{encode_tag, language_pair_bytes};

fn read_json(filename: &str) -> Result<json::JsonValue, Error> {
    let mut f = File::open(filename)?;
    let mut target_str = String::new();
    f.read_to_string(&mut target_str)?;
    Ok(json::parse(&target_str).unwrap())
}

fn make_tables() -> Result<(), Error> {
    let out_path = Path::new(&env::var("OUT_DIR").unwrap()).join("langdata.rs");
    let mut out_file = BufWriter::new(File::create(&out_path)?);

    let parsed = read_json("data/aliases.json")?;
    let ref language_aliases = parsed["supplemental"]["metadata"]["alias"]["languageAlias"];
    let mut builder = phf_codegen::Map::new();

    // Handle replacements of entire language tags, based on string matching
    write!(&mut out_file,
           "pub static TAG_REPLACE: ::phf::Map<&'static str, u64> = ")?;
    for pair in language_aliases.entries() {
        let (key, val) = pair;
        let replacement = encode_tag(&val["_replacement"].to_string()).unwrap();
        // let key_lower: &'static str = &key.to_lowercase();
        builder.entry(key.to_lowercase(), &replacement.to_string());
    }
    builder.build(&mut out_file).unwrap();
    write!(&mut out_file, ";\n")?;

    // Handle replacements for the language tag in particular.
    let mut builder = phf_codegen::Map::new();
    write!(&mut out_file,
           "pub static LANG_REPLACE: ::phf::Map<u64, u64> = ")?;
    for pair in language_aliases.entries() {
        let (key, val) = pair;
        if !key.contains("-") {
            let replaced = encode_tag(key).unwrap();
            let replacement = encode_tag(&val["_replacement"].to_string()).unwrap();
            builder.entry(replaced, &replacement.to_string());
        }
    }
    builder.build(&mut out_file).unwrap();
    write!(&mut out_file, ";\n")?;

    // Handle region replacements, which are simpler than language tag
    // replacements.
    let ref region_aliases = parsed["supplemental"]["metadata"]["alias"]["territoryAlias"];
    let mut builder = phf_codegen::Map::new();
    write!(&mut out_file,
           "pub static REGION_REPLACE: ::phf::Map<u64, u64> = ")?;
    for pair in region_aliases.entries() {
        let (key, val) = pair;
        let replace_val = val["_replacement"].to_string();
        // Skip replacements with spaces; these indicate multiple
        // possibilities, such as replacing Yugoslavia with its
        // successors. It is extremely unclear how to handle this case.
        if !replace_val.contains(" ") {
            if key.len() == 2 || key.chars().nth(0).unwrap().is_digit(10) {
                let replaced = encode_tag(&format!("und-{}", key)).unwrap();
                let replacement = encode_tag(&format!("und-{}", replace_val)).unwrap();
                builder.entry(replaced, &replacement.to_string());
            }
        }
    }
    builder.build(&mut out_file).unwrap();
    write!(&mut out_file, ";\n")?;

    let parsed = read_json("data/likelySubtags.json")?;
    let ref likely_subtags = parsed["supplemental"]["likelySubtags"];
    let mut builder = phf_codegen::Map::new();
    write!(&mut out_file,
           "pub static LIKELY_SUBTAGS: ::phf::Map<u64, u64> = ")?;
    for pair in likely_subtags.entries() {
        let (key, val) = pair;
        let from_tag = encode_tag(key).unwrap();
        let to_tag = encode_tag(&val.to_string()).unwrap();
        builder.entry(from_tag, &to_tag.to_string());
    }
    builder.build(&mut out_file).unwrap();
    write!(&mut out_file, ";\n")?;

    // Read a file of language matches
    let in_file = try!(File::open("data/matching.txt"));
    let in_buf = BufReader::new(&in_file);
    let mut builder = phf_codegen::Map::new();
    write!(&mut out_file,
           "pub static MATCH_DISTANCE: ::phf::Map<[u8; 16], i32> = ")?;
    for line_w in in_buf.lines() {
        let line = line_w?;
        let parts: Vec<&str> = line.split(",").collect();
        let lang1 = encode_tag(parts[0]).unwrap();
        let lang2 = encode_tag(parts[1]).unwrap();
        let distance: i32 = parts[2].parse().unwrap();
        let sym: bool = parts[3] == "sym";
        let pair1 = language_pair_bytes(lang1, lang2);
        let pair2 = language_pair_bytes(lang2, lang1);
        builder.entry(pair1, &distance.to_string());
        if sym {
            builder.entry(pair2, &distance.to_string());
        }
    }
    builder.build(&mut out_file).unwrap();
    write!(&mut out_file, ";\n")?;

    // Now write a convenient file of constants for commonly-used languages.
    let const_path = Path::new(&env::var("OUT_DIR").unwrap()).join("languages.rs");
    let mut const_file = BufWriter::new(File::create(&const_path)?);
    let in_file = try!(File::open("data/languages.txt"));
    let in_buf = BufReader::new(&in_file);
    for line_w in in_buf.lines() {
        let line = line_w?;
        let parts: Vec<&str> = line.split("\t").collect();
        let from_name = parts[0];
        let to_code = encode_tag(parts[1]).unwrap();
        write!(&mut const_file,
               "pub const {:<24}: LanguageCode = LanguageCode {{ data: 0x{:>016x}_u64 }};\n",
               from_name,
               to_code)?;
    }

    Ok(())
}

fn main() {
    make_tables().unwrap();
}
