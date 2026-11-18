//! Generates the PHP constant classes for Bedrock block states.
//!
//! `data/canonical_block_states.nbt` is the same palette `bedrock_world`
//! parses at startup (`bedrock_world::block_states`), so generating from it
//! keeps `BlockTypeNames`, `BlockStateNames` and `BlockStateStringValues` in
//! sync with the server instead of hand-maintaining a second copy.
//!
//! Output goes to `OUT_DIR`; `src/runtime/bootstrap.rs` embeds the files into
//! `PHP_RUNTIME_FILES`.
//!
//! Encoding: the file holds consecutive network-NBT roots (`NetworkNbtSerializer`
//! layout: little-endian shorts/floats, zigzag-varint ints/longs, varuint string
//! lengths), each a compound with `name` (string) and `states` (compound of
//! properties, including `version`).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

const PALETTE: &str = "../../data/canonical_block_states.nbt";

const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_SHORT: u8 = 2;
const TAG_INT: u8 = 3;
const TAG_LONG: u8 = 4;
const TAG_FLOAT: u8 = 5;
const TAG_DOUBLE: u8 = 6;
const TAG_BYTE_ARRAY: u8 = 7;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;
const TAG_INT_ARRAY: u8 = 11;
const TAG_LONG_ARRAY: u8 = 12;

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn done(&self) -> bool {
        self.pos >= self.data.len()
    }

    fn bytes(&mut self, len: usize) -> &'a [u8] {
        assert!(self.pos + len <= self.data.len(), "truncated canonical palette");
        let slice = &self.data[self.pos..self.pos + len];
        self.pos += len;
        slice
    }

    fn u8(&mut self) -> u8 {
        self.bytes(1)[0]
    }

    fn varuint(&mut self) -> u64 {
        let mut value = 0u64;
        for shift in 0..10 {
            let byte = self.u8();
            value |= u64::from(byte & 0x7f) << (shift * 7);
            if byte & 0x80 == 0 {
                return value;
            }
        }
        panic!("malformed varuint in canonical palette");
    }

    fn zigzag32(&mut self) -> i32 {
        let raw = self.varuint() as u32;
        ((raw >> 1) as i32) ^ -((raw & 1) as i32)
    }

    fn string(&mut self) -> String {
        let len = self.varuint() as usize;
        String::from_utf8(self.bytes(len).to_vec()).expect("non-UTF-8 string in palette")
    }

    fn skip_payload(&mut self, tag: u8) {
        match tag {
            TAG_END => {}
            TAG_BYTE => {
                self.bytes(1);
            }
            TAG_SHORT => {
                self.bytes(2);
            }
            TAG_INT | TAG_LONG => {
                self.varuint();
            }
            TAG_FLOAT => {
                self.bytes(4);
            }
            TAG_DOUBLE => {
                self.bytes(8);
            }
            TAG_BYTE_ARRAY => {
                let len = self.zigzag32().max(0) as usize;
                self.bytes(len);
            }
            TAG_STRING => {
                let len = self.varuint() as usize;
                self.bytes(len);
            }
            TAG_LIST => {
                let element = self.u8();
                let len = self.zigzag32().max(0) as usize;
                for _ in 0..len {
                    self.skip_payload(element);
                }
            }
            TAG_COMPOUND => loop {
                let inner = self.u8();
                if inner == TAG_END {
                    break;
                }
                let _ = self.string();
                self.skip_payload(inner);
            },
            TAG_INT_ARRAY | TAG_LONG_ARRAY => {
                let len = self.zigzag32().max(0) as usize;
                for _ in 0..len {
                    self.varuint();
                }
            }
            other => panic!("unknown NBT tag {other} in canonical palette"),
        }
    }
}

#[derive(Default)]
struct Palette {
    blocks: BTreeSet<String>,
    /// property name -> string values it can take
    properties: BTreeMap<String, BTreeSet<String>>,
    version: i32,
}

fn parse(data: &[u8]) -> Palette {
    let mut reader = Reader::new(data);
    let mut palette = Palette::default();

    while !reader.done() {
        let tag = reader.u8();
        assert_eq!(tag, TAG_COMPOUND, "canonical palette root must be a compound");
        let _root_name = reader.string();

        let mut name: Option<String> = None;
        loop {
            let inner = reader.u8();
            if inner == TAG_END {
                break;
            }
            let key = reader.string();
            match (key.as_str(), inner) {
                ("name", TAG_STRING) => name = Some(reader.string()),
                // The palette version lives at the root, next to `states`.
                ("version", TAG_INT) => {
                    palette.version = reader.zigzag32();
                }
                ("states", TAG_COMPOUND) => loop {
                    let state_tag = reader.u8();
                    if state_tag == TAG_END {
                        break;
                    }
                    let property = reader.string();
                    if property == "version" && state_tag == TAG_INT {
                        palette.version = reader.zigzag32();
                        continue;
                    }
                    let values = palette.properties.entry(property).or_default();
                    if state_tag == TAG_STRING {
                        values.insert(reader.string());
                    } else {
                        reader.skip_payload(state_tag);
                    }
                },
                _ => reader.skip_payload(inner),
            }
        }

        if let Some(name) = name {
            palette.blocks.insert(name);
        }
    }

    palette
}

fn ident(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_uppercase());
        } else if !out.ends_with('_') {
            out.push('_');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    match trimmed.chars().next() {
        None => String::new(),
        Some(first) if first.is_ascii_digit() => format!("_{trimmed}"),
        Some(_) => trimmed,
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let palette_path = manifest_dir.join(PALETTE);
    println!("cargo:rerun-if-changed={}", palette_path.display());

    let data = fs::read(&palette_path).expect("read canonical_block_states.nbt");
    let palette = parse(&data);
    assert!(!palette.blocks.is_empty(), "empty canonical palette");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    // BlockTypeNames: one constant per block, `minecraft:` stripped like PM.
    let mut seen_blocks: BTreeMap<String, String> = BTreeMap::new();
    let mut body = String::new();
    for name in &palette.blocks {
        let bare = name.strip_prefix("minecraft:").unwrap_or(name);
        let const_name = ident(bare);
        if const_name.is_empty() {
            continue;
        }
        if let Some(previous) = seen_blocks.get(&const_name) {
            assert_eq!(previous, name, "block constant collision on {const_name}");
            continue;
        }
        seen_blocks.insert(const_name.clone(), name.clone());
        writeln!(body, "        public const {const_name} = \"{}\";", escape(name)).unwrap();
    }
    write_class(&out_dir, "block_type_names.php", "BlockTypeNames", &body);

    // BlockStateNames: one constant per property name.
    let mut seen_props: BTreeMap<String, String> = BTreeMap::new();
    let mut body = String::new();
    for property in palette.properties.keys() {
        let const_name = ident(property);
        if const_name.is_empty() {
            continue;
        }
        if let Some(previous) = seen_props.get(&const_name) {
            assert_eq!(previous, property, "property constant collision on {const_name}");
            continue;
        }
        seen_props.insert(const_name.clone(), property.clone());
        writeln!(body, "        public const {const_name} = \"{}\";", escape(property)).unwrap();
    }
    write_class(&out_dir, "block_state_names.php", "BlockStateNames", &body);

    // BlockStateStringValues: `<PROPERTY>_<VALUE>` per enum value.
    let mut seen_values: BTreeMap<String, String> = BTreeMap::new();
    let mut body = String::new();
    for (property, values) in &palette.properties {
        let property_ident = ident(property);
        for value in values {
            let value_ident = ident(value);
            if property_ident.is_empty() || value_ident.is_empty() {
                continue;
            }
            let const_name = format!("{property_ident}_{value_ident}");
            let raw = format!("{property}={value}");
            if let Some(previous) = seen_values.get(&const_name) {
                assert_eq!(*previous, raw, "value constant collision on {const_name}");
                continue;
            }
            seen_values.insert(const_name.clone(), raw);
            writeln!(body, "        public const {const_name} = \"{}\";", escape(value)).unwrap();
        }
    }
    write_class(&out_dir, "block_state_string_values.php", "BlockStateStringValues", &body);

    let version = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace pocketmine\\data\\bedrock\\block {{\n\n    /**\n     * Version of the Bedrock block-state palette the server ships\n     * (`data/canonical_block_states.nbt`), generated by build.rs.\n     */\n    final class BlockStateVersion\n    {{\n        public const PALETTE_VERSION = {};\n    }}\n}}\n",
        palette.version
    );
    fs::write(out_dir.join("block_state_version.php"), version).expect("write block_state_version.php");
}

fn write_class(out_dir: &PathBuf, file: &str, class: &str, body: &str) {
    let source = format!(
        "<?php\n\ndeclare(strict_types=1);\n\nnamespace pocketmine\\data\\bedrock\\block {{\n\n    /**\n     * Generated by `crates/php-plugin-bridge/build.rs` from\n     * `data/canonical_block_states.nbt`; do not edit by hand.\n     */\n    final class {class}\n    {{\n{body}    }}\n}}\n"
    );
    fs::write(out_dir.join(file), source).expect("write generated constant class");
}
