//! Bedrock JSON NBT, the syntax `/give [...tags]` accepts.
//!
//! PocketMine parses the argument with `JsonNbtParser::parseJson()`, which
//! infers the tag type from the JSON value: booleans become `TAG_Byte`,
//! integers `TAG_Int`, decimals `TAG_Double`, strings `TAG_String`, objects
//! `TAG_Compound` and arrays `TAG_List`. The root must be an object, since the
//! result is the item's named tag.
//!
//! The messages produced here are our own wording: the parser lives in the
//! `pocketmine/nbt` composer package, which is not vendored in `target/`, so
//! only the translation key and its `{%0}` slot are copied from PM.

use bedrock_protocol::nbt::Tag;
use serde_json::Value;

/// Parses a JSON NBT document into a root tag.
pub fn parse(json: &str) -> Result<Tag, String> {
    let value: Value =
        serde_json::from_str(json).map_err(|err| format!("Invalid JSON NBT: {err}"))?;
    let Value::Object(entries) = value else {
        return Err("Invalid JSON NBT: the root tag must be a compound".to_string());
    };
    Ok(Tag::Compound(parse_entries(entries)?))
}

fn parse_entries(
    entries: serde_json::Map<String, Value>,
) -> Result<Vec<(String, Tag)>, String> {
    entries
        .into_iter()
        .map(|(name, value)| Ok((name, parse_value(value)?)))
        .collect()
}

fn parse_value(value: Value) -> Result<Tag, String> {
    Ok(match value {
        Value::Null => return Err("Invalid JSON NBT: null is not a tag".to_string()),
        // `JsonNbtParser` maps booleans to TAG_Byte, like Java NBT.
        Value::Bool(flag) => Tag::Byte(flag as i8),
        Value::Number(number) => {
            if let Some(integer) = number.as_i64() {
                let integer = i32::try_from(integer).map_err(|_| {
                    format!("Invalid JSON NBT: {integer} does not fit in a TAG_Int")
                })?;
                Tag::Int(integer)
            } else if let Some(decimal) = number.as_f64() {
                Tag::Double(decimal)
            } else {
                return Err(format!("Invalid JSON NBT: unsupported number {number}"));
            }
        }
        Value::String(text) => Tag::String(text),
        Value::Array(items) => {
            let items = items
                .into_iter()
                .map(parse_value)
                .collect::<Result<Vec<_>, String>>()?;
            // NBT lists are homogeneous; a mixed list cannot be serialized.
            if let Some(first) = items.first() {
                if items.iter().any(|item| item.type_id() != first.type_id()) {
                    return Err(
                        "Invalid JSON NBT: a list must contain a single tag type".to_string()
                    );
                }
            }
            Tag::List(items)
        }
        Value::Object(entries) => Tag::Compound(parse_entries(entries)?),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_tags_from_json_values() {
        let tag = parse(
            r#"{"display":{"Name":"Excalibur"},"Damage":3,"Unbreakable":true}"#,
        )
        .expect("valid JSON NBT");

        // `serde_json` keeps object keys sorted, so `Damage` and `Unbreakable`
        // come before `display`; compound order carries no meaning in NBT.
        assert_eq!(
            tag,
            Tag::Compound(vec![
                ("Damage".to_string(), Tag::Int(3)),
                ("Unbreakable".to_string(), Tag::Byte(1)),
                (
                    "display".to_string(),
                    Tag::Compound(vec![(
                        "Name".to_string(),
                        Tag::String("Excalibur".to_string())
                    )])
                ),
            ])
        );
    }

    #[test]
    fn decimals_become_doubles() {
        assert_eq!(
            parse(r#"{"speed":1.5}"#).unwrap(),
            Tag::Compound(vec![("speed".to_string(), Tag::Double(1.5))])
        );
    }

    #[test]
    fn rejects_a_non_compound_root() {
        assert!(parse(r#"[1,2]"#).unwrap_err().contains("root tag must be a compound"));
        assert!(parse("true").is_err());
    }

    #[test]
    fn rejects_malformed_json_and_mixed_lists() {
        assert!(parse(r#"{"a":"#).is_err());
        assert!(parse(r#"{"a":[1,"two"]}"#)
            .unwrap_err()
            .contains("single tag type"));
    }
}
