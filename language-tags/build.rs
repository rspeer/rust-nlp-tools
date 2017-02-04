extern crate phf_codegen;
extern crate json;

use std::env;
use std::path::Path;
use std::io::prelude::*;
use std::io::{BufWriter, Error};
use std::fs::File;


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

    // Handle language tag replacements, which are used both as replacements
    // for the language subtag and for the entire tag. If they replace the
    // language subtag, they may need to be re-parsed. Make them lowercase,
    // so that they're already normalized for the parser.
    write!(&mut out_file, "extern crate phf;\n").unwrap();
    write!(&mut out_file,
           "pub static LANG_REPLACE: phf::Map<&'static str, &'static str> = ")?;
    for pair in language_aliases.entries() {
        let (key, val) = pair;
        let replacement = val["_replacement"].to_string().to_lowercase();
        let val_literal = format!("\"{}\"", replacement);
        builder.entry(key.to_lowercase(), &val_literal);
    }
    builder.build(&mut out_file).unwrap();
    write!(&mut out_file, ";\n")?;

    // Handle region replacements, which are simpler than language tag
    // replacements.
    let ref region_aliases = parsed["supplemental"]["metadata"]["alias"]["territoryAlias"];
    let mut builder = phf_codegen::Map::new();
    write!(&mut out_file,
           "pub static REGION_REPLACE: phf::Map<&'static str, &'static str> = ")?;
    for pair in region_aliases.entries() {
        let (key, val) = pair;
        let replacement = val["_replacement"].to_string();
        // Skip replacements with spaces; these indicate multiple
        // possibilities, such as replacing the Soviet Union with its
        // successors. It is extremely unclear how to handle this case.
        if !replacement.contains(" ") {
            if key.len() == 2 || key.chars().nth(0).unwrap().is_digit(10) {
                let val_literal = format!("\"{}\"", replacement);
                builder.entry(key.to_uppercase(), &val_literal);
            }
        }
    }
    builder.build(&mut out_file).unwrap();
    write!(&mut out_file, ";\n")?;

    Ok(())
}

fn main() {
    make_tables().unwrap();
}
