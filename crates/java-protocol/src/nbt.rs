//! Byte-for-byte port of `plugins/JavaBridge/src/javabridge/Nbt.php`:
//! a writer for Java network ("anonymous") NBT — type byte + payload, no
//! root name — over the typed-JSON node shape used by protodef and the
//! registry files:
//!
//! ```json
//! {"type":"compound","value":{"ke":{"type":"int","value":1}}}
//! {"type":"list","value":{"type":"string","value":["a","b"]}}
//! ```
//!
//! Longs accept either a JSON integer or a `[high, low]` pair (protodef
//! JavaScript serialization); see `long_value`. All integers and
//! IEEE floats are big-endian (`pack('N'|'n'|'E'|'G'|'J'|'n'`), matching
//! the vectors in `target/nbtcheck/expected.json` (prismarine-nbt).
//!
//! The Java flavor differs completely from the Bedrock network NBT in
//! `crates/bedrock-protocol/src/nbt.rs` (little-endian + zigzag varints);
//! the two must not be conflated.
//!
//! Output must stay in insertion order to be byte-identical: the crate
//! enables serde_json's `preserve_order` feature so compound iteration
//! follows the JSON file order, exactly like PHP arrays.

use serde_json::Value;
use thiserror::Error;

pub const TAG_END: u8 = 0;
pub const TAG_BYTE: u8 = 1;
pub const TAG_SHORT: u8 = 2;
pub const TAG_INT: u8 = 3;
pub const TAG_LONG: u8 = 4;
pub const TAG_FLOAT: u8 = 5;
pub const TAG_DOUBLE: u8 = 6;
pub const TAG_BYTE_ARRAY: u8 = 7;
pub const TAG_STRING: u8 = 8;
pub const TAG_LIST: u8 = 9;
pub const TAG_COMPOUND: u8 = 10;
pub const TAG_INT_ARRAY: u8 = 11;
pub const TAG_LONG_ARRAY: u8 = 12;

/// Failures matching PHP's `InvalidArgumentException` for unknown types.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NbtError {
    #[error("Tipo NBT desconocido: {0}")]
    UnknownType(String),
}

fn tag_of(kind: &str) -> Result<u8, NbtError> {
    match kind {
        "end" => Ok(TAG_END),
        "byte" => Ok(TAG_BYTE),
        "short" => Ok(TAG_SHORT),
        "int" => Ok(TAG_INT),
        "long" => Ok(TAG_LONG),
        "float" => Ok(TAG_FLOAT),
        "double" => Ok(TAG_DOUBLE),
        "byteArray" => Ok(TAG_BYTE_ARRAY),
        "string" => Ok(TAG_STRING),
        "list" => Ok(TAG_LIST),
        "compound" => Ok(TAG_COMPOUND),
        "intArray" => Ok(TAG_INT_ARRAY),
        "longArray" => Ok(TAG_LONG_ARRAY),
        other => Err(NbtError::UnknownType(other.to_string())),
    }
}

fn node_type(node: &Value) -> Result<&str, NbtError> {
    node.get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| NbtError::UnknownType("<missing type>".to_string()))
}

fn node_value<'a>(node: &'a Value) -> Option<&'a Value> {
    node.get("value")
}

fn as_i64(value: Option<&Value>) -> i64 {
    value.and_then(Value::as_i64).unwrap_or(0)
}

fn as_f64(value: Option<&Value>) -> f64 {
    value.and_then(Value::as_f64).unwrap_or(0.0)
}

fn nbt_string(value: &str) -> Vec<u8> {
    let mut bytes = ((value.len() & 0xFFFF) as u16).to_be_bytes().to_vec();
    bytes.extend_from_slice(value.as_bytes());
    bytes
}

/// Long value as a signed 64-bit integer, from a JSON integer or a
/// `[high, low]` pair. Mirrors `Nbt::longValue`.
fn long_value(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Array(pair)) if pair.len() >= 2 => {
            let high = pair[0].as_i64().unwrap_or(0);
            let low = pair[1].as_i64().unwrap_or(0);
            (((high as u32) as u64) << 32 | ((low as u32) as u64)) as i64
        }
        Some(Value::Array(pair)) if pair.len() == 1 => {
            let high = pair[0].as_i64().unwrap_or(0);
            (((high as u32) as u64) << 32) as i64
        }
        other => other.and_then(Value::as_i64).unwrap_or(0),
    }
}

/// Serialize a typed-JSON node as network NBT: type byte + payload.
pub fn anonymous(node: &Value) -> Result<Vec<u8>, NbtError> {
    let kind = node_type(node)?;
    let mut out = vec![tag_of(kind)?];
    out.extend(payload(node)?);
    Ok(out)
}

/// Plain-text component `{"text": ...}` as an anonymous compound.
/// Mirrors `Nbt::text`.
pub fn text(text: &str) -> Vec<u8> {
    // Infallible: the node shape is fixed and valid.
    anonymous(&serde_json::json!({
        "type": "compound",
        "value": { "text": { "type": "string", "value": text } }
    }))
    .expect("text component node is valid")
}

/// Compound with a single long array (heightmaps). Mirrors
/// `Nbt::longArrayCompound`.
pub fn long_array_compound(key: &str, values: &[i64]) -> Vec<u8> {
    let entries: Vec<Value> = values.iter().map(|v| serde_json::json!(*v)).collect();
    anonymous(&serde_json::json!({
        "type": "compound",
        "value": { key: { "type": "longArray", "value": entries } }
    }))
    .expect("long array compound node is valid")
}

fn payload(node: &Value) -> Result<Vec<u8>, NbtError> {
    let kind = node_type(node)?;
    let value = node_value(node);

    if kind == "end" {
        return Ok(Vec::new());
    }

    if kind == "compound" {
        let mut out = Vec::new();
        if let Some(map) = value.and_then(Value::as_object) {
            // serde_json with preserve_order iterates in file order,
            // like the PHP array.
            for (name, child) in map {
                let child_kind = node_type(child)?;
                out.push(tag_of(child_kind)?);
                out.extend(nbt_string(name));
                out.extend(payload(child)?);
            }
        }
        out.push(TAG_END);
        return Ok(out);
    }

    if kind == "list" {
        let inner_empty = Value::Object(Default::default());
        let inner = value.unwrap_or(&inner_empty);
        let element_kind = inner.get("type").and_then(Value::as_str).unwrap_or("end");
        let elements = inner.get("value").and_then(Value::as_array);
        let elements: &[Value] = elements.map(Vec::as_slice).unwrap_or(&[]);
        let mut out = vec![tag_of(element_kind)?];
        out.extend((elements.len() as u32).to_be_bytes());
        for element in elements {
            if element_kind == "compound" {
                out.extend(payload(&serde_json::json!({
                    "type": "compound",
                    "value": element
                }))?);
            } else {
                out.extend(payload(&serde_json::json!({
                    "type": element_kind,
                    "value": element
                }))?);
            }
        }
        return Ok(out);
    }

    if kind == "byte" {
        return Ok(vec![(as_i64(value) & 0xFF) as u8]);
    }
    if kind == "short" {
        return Ok(((as_i64(value) & 0xFFFF) as u16).to_be_bytes().to_vec());
    }
    if kind == "int" {
        return Ok(((as_i64(value) as i32) as u32).to_be_bytes().to_vec());
    }
    if kind == "long" {
        return Ok(long_value(value).to_be_bytes().to_vec());
    }
    if kind == "float" {
        return Ok((as_f64(value) as f32).to_be_bytes().to_vec());
    }
    if kind == "double" {
        return Ok(as_f64(value).to_be_bytes().to_vec());
    }
    if kind == "string" {
        let s = value.and_then(Value::as_str).unwrap_or("");
        return Ok(nbt_string(s));
    }
    if kind == "byteArray" {
        let bytes: Vec<u8> = match value {
            Some(Value::Array(items)) => items
                .iter()
                .map(|b| (b.as_i64().unwrap_or(0) & 0xFF) as u8)
                .collect(),
            Some(Value::String(s)) => s.as_bytes().to_vec(),
            _ => Vec::new(),
        };
        let mut out = (bytes.len() as u32).to_be_bytes().to_vec();
        out.extend_from_slice(&bytes);
        return Ok(out);
    }
    if kind == "intArray" {
        let items = value.and_then(Value::as_array);
        let items: &[Value] = items.map(Vec::as_slice).unwrap_or(&[]);
        let mut out = (items.len() as u32).to_be_bytes().to_vec();
        for item in items {
            out.extend(((item.as_i64().unwrap_or(0) as i32) as u32).to_be_bytes());
        }
        return Ok(out);
    }
    if kind == "longArray" {
        let items = value.and_then(Value::as_array);
        let items: &[Value] = items.map(Vec::as_slice).unwrap_or(&[]);
        let mut out = (items.len() as u32).to_be_bytes().to_vec();
        for item in items {
            out.extend(long_value(Some(item)).to_be_bytes());
        }
        return Ok(out);
    }

    Err(NbtError::UnknownType(kind.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn text_component_vector() {
        assert_eq!(
            hex(&text("Hola mundo")),
            "0a08000474657874000a486f6c61206d756e646f00"
        );
        assert_eq!(hex(&text("")), "0a08000474657874000000");
    }

    #[test]
    fn mixed_compound_vector() {
        // From target/nbtcheck/validate.js ("mixed") via Nbt::anonymous.
        let node = json!({"type":"compound","value":{
            "b":{"type":"byte","value":-1},
            "s":{"type":"short","value":-300},
            "i":{"type":"int","value":-70000},
            "l":{"type":"long","value":[-2,5]},
            "f":{"type":"float","value":0.5},
            "d":{"type":"double","value":-1.25},
            "str":{"type":"string","value":"\u{fffd}and\u{fffd}"},
            "empty":{"type":"list","value":{"type":"end","value":[]}},
            "strings":{"type":"list","value":{"type":"string","value":["a","bb"]}},
            "compounds":{"type":"list","value":{"type":"compound","value":[{"x":{"type":"int","value":1}},{"x":{"type":"int","value":2}}]}},
            "ints":{"type":"intArray","value":[1,-2,3]},
            "bytes":{"type":"byteArray","value":[1,2,-1]}
        }});
        assert_eq!(
            hex(&anonymous(&node).unwrap()),
            "0a01000162ff02000173fed403000169fffeee900400016cfffffffe00000005050001663f00000006000164bff40000000000000800037374720009efbfbd616e64efbfbd090005656d7074790000000000090007737472696e6773080000000200016100026262090009636f6d706f756e64730a000000020300017800000001000300017800000002000b0004696e74730000000300000001fffffffe000000030700056279746573000000030102ff00"
        );
    }

    #[test]
    fn heightmap_vector() {
        let zeros = vec![json!([0, 0]); 37];
        let node = json!({"type":"compound","value":{
            "MOTION_BLOCKING":{"type":"longArray","value":zeros}
        }});
        let bytes = anonymous(&node).unwrap();
        assert_eq!(bytes.len(), 320);
        assert_eq!(bytes[0], 0x0A);
        assert_eq!(bytes[1], 0x0C);
        assert_eq!(&bytes[2..4], &[0x00, 0x0F]);
        assert_eq!(&bytes[4..19], b"MOTION_BLOCKING");
        assert_eq!(&bytes[19..23], &[0x00, 0x00, 0x00, 0x25]);
        assert!(bytes[23..319].iter().all(|&b| b == 0));
        assert_eq!(bytes[319], 0x00);
    }

    #[test]
    fn long_array_pair_vector() {
        let node = json!({"type":"compound","value":{
            "values":{"type":"longArray","value":[[0,1],[-1,-1],[1,0]]}
        }});
        assert_eq!(
            hex(&anonymous(&node).unwrap()),
            "0a0c000676616c756573000000030000000000000001ffffffffffffffff000000010000000000"
        );
    }

    #[test]
    fn unknown_type_errors() {
        let node = json!({"type":"nope","value":1});
        assert_eq!(
            anonymous(&node),
            Err(NbtError::UnknownType("nope".to_string()))
        );
        assert!(matches!(
            anonymous(&json!({"type":"compound","value":{"k":{"type":"nope","value":1}}})),
            Err(NbtError::UnknownType(_))
        ));
    }

    #[test]
    fn byte_array_accepts_string_bytes() {
        let node = json!({"type":"byteArray","value":"ab"});
        assert_eq!(hex(&anonymous(&node).unwrap()), "07000000026162");
    }
}
