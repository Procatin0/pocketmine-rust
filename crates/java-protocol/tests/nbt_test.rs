//! Integration tests ported from `plugins/JavaBridge/tests/nbt_test.php`
//! (which compares `Nbt::anonymous` against prismarine-nbt reference
//! binaries in `target/nbtcheck/expected.json`).
//!
//! The PHP test reads the full manifest (1471 cases: every registry entry
//! plus the `text` / `heightmap` / `longArray` / `mixed` cases from
//! `target/nbtcheck/validate.js`). The manifest is a build artifact, not a
//! tracked file, so the vectors below hardcode the hand-written cases from
//! `validate.js` (including `mixed`) plus the first registry entry
//! (`769.json:minecraft:badlands`), all captured hex-for-hex from the PHP
//! implementation. The unit tests in `src/nbt.rs` hold the same vectors;
//! the per-entry sweep over all 1471 cases reproduces exactly when the
//! Rust binary is run against a fresh `expected.json`.

use java_protocol::nbt;
use serde_json::json;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn text_case() {
    assert_eq!(
        hex(&nbt::text("Hola mundo")),
        "0a08000474657874000a486f6c61206d756e646f00"
    );
}

#[test]
fn heightmap_case() {
    let zeros = vec![json!([0, 0]); 37];
    let node = json!({"type":"compound","value":{
        "MOTION_BLOCKING":{"type":"longArray","value":zeros}
    }});
    let bytes = nbt::anonymous(&node).unwrap();
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
fn long_array_pair_case() {
    let node = json!({"type":"compound","value":{
        "values":{"type":"longArray","value":[[0,1],[-1,-1],[1,0]]}
    }});
    assert_eq!(
        hex(&nbt::anonymous(&node).unwrap()),
        "0a0c000676616c756573000000030000000000000001ffffffffffffffff000000010000000000"
    );
}

#[test]
fn mixed_case() {
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
        hex(&nbt::anonymous(&node).unwrap()),
        "0a01000162ff02000173fed403000169fffeee900400016cfffffffe00000005050001663f00000006000164bff40000000000000800037374720009efbfbd616e64efbfbd090005656d7074790000000000090007737472696e6773080000000200016100026262090009636f6d706f756e64730a000000020300017800000001000300017800000002000b0004696e74730000000300000001fffffffe000000030700056279746573000000030102ff00"
    );
}

#[test]
fn first_registry_entry_case() {
    // 769.json:minecraft:badlands (the first case in expected.json).
    let raw = include_str!("../resources/registry/769.json");
    let decoded: serde_json::Value = serde_json::from_str(raw).unwrap();
    let first = &decoded["registries"][0]["entries"][0];
    assert_eq!(first["key"], json!("minecraft:badlands"));
    assert_eq!(
        hex(&nbt::anonymous(&first["value"]).unwrap()),
        "0a0a0007656666656374730a00056d757369630100157265706c6163655f63757272656e745f6d75736963000300096d61785f64656c617900005dc0080005736f756e6400226d696e6563726166743a6d757369632e6f766572776f726c642e6261646c616e64730300096d696e5f64656c617900002ee000030009736b795f636f6c6f72006eb1ff03000b67726173735f636f6c6f720090814d03000d666f6c696167655f636f6c6f72009e814d03000f77617465725f666f675f636f6c6f7200050533030009666f675f636f6c6f7200c0d8ff03000b77617465725f636f6c6f72003f76e40a000a6d6f6f645f736f756e6403000a7469636b5f64656c6179000017700600066f66667365744000000000000000080005736f756e6400166d696e6563726166743a616d6269656e742e63617665030013626c6f636b5f7365617263685f657874656e740000000800000100116861735f70726563697069746174696f6e0005000b74656d706572617475726540000000050008646f776e66616c6c0000000000"
    );
}

#[test]
fn byte_array_string_case() {
    let node = json!({"type":"byteArray","value":"ab"});
    assert_eq!(hex(&nbt::anonymous(&node).unwrap()), "07000000026162");
}
