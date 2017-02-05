extern crate phf_codegen;
extern crate language_tag_parser;
extern crate json;

use std::env;
use std::path::Path;
use std::io::prelude::*;
use std::io::{BufWriter, BufReader, Error};
use std::fs::File;
use language_tag_parser::parse_tag;

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
        let replacement = parse_tag(&val["_replacement"].to_string()).unwrap();
        builder.entry(key, &replacement.to_string());
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
            let replaced = parse_tag(key).unwrap();
            let replacement = parse_tag(&val["_replacement"].to_string()).unwrap();
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
           "pub static REGION_REPLACE: ::phf::Map<&'static str, u64> = ")?;
    for pair in region_aliases.entries() {
        let (key, val) = pair;
        let replace_val = val["_replacement"].to_string();
        // Skip replacements with spaces; these indicate multiple
        // possibilities, such as replacing Yugoslavia with its
        // successors. It is extremely unclear how to handle this case.
        if !replace_val.contains(" ") {
            if key.len() == 2 || key.chars().nth(0).unwrap().is_digit(10) {
                let replaced = parse_tag(&format!("und-{}", key)).unwrap();
                let replacement = parse_tag(&format!("und-{}", replace_val)).unwrap();
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
        let from_tag = parse_tag(key).unwrap();
        let to_tag = parse_tag(&val.to_string()).unwrap();
        builder.entry(from_tag, &to_tag.to_string());
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
        if parts.len() < 2 {
            println!("{}", line);
        }
        let from_name = parts[0];
        let to_code = parse_tag(parts[1]).unwrap();
        write!(&mut const_file,
               "pub const {}: u64 = 0x{:x}_u64;\n",
               from_name,
               to_code)?;
    }

    Ok(())
}

fn main() {
    make_tables().unwrap();
}
