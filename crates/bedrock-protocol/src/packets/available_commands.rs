use crate::binary::Writer;

/// Permission string used for player-usable commands in protocol 2225.
pub const PERMISSION_ANY: &str = "any";

/// `ARG_FLAG_VALID`: marks a parameter type as valid.
const ARG_FLAG_VALID: u32 = 0x100_000;
/// Free-form text argument (`ARG_TYPE_RAWTEXT`).
const ARG_TYPE_RAWTEXT: u32 = 70;

/// A command advertised to the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
}

/// Packet ID 0x4C (76): AvailableCommandsPacket.
///
/// Layout follows protocol 2225 (Minecraft 1.26.60), the same one Axolotl uses:
/// enum values, chained subcommand values, postfixes, hard enums, chained
/// subcommand data, command data, soft enums and enum constraints.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AvailableCommandsPacket {
    pub commands: Vec<CommandSpec>,
}

impl AvailableCommandsPacket {
    pub const PACKET_ID: u32 = 0x4C;

    /// Writes the payload; the packet id is written by the caller because
    /// server-core emits this as raw wire bytes.
    pub fn encode_payload(&self, writer: &mut Writer) {
        let mut enum_values: Vec<String> = Vec::new();
        let mut enums: Vec<(String, Vec<u32>)> = Vec::new();
        let mut alias_indexes: Vec<i32> = Vec::with_capacity(self.commands.len());

        for command in &self.commands {
            if command.aliases.is_empty() {
                alias_indexes.push(-1);
                continue;
            }

            // Aliases plus the label itself, working around the client bug that
            // hides the original name when aliases are present.
            let label = command.name.to_lowercase();
            let mut values: Vec<String> =
                command.aliases.iter().map(|alias| alias.to_lowercase()).collect();
            if !values.iter().any(|value| *value == label) {
                values.push(label);
            }

            let mut indexes = Vec::with_capacity(values.len());
            for value in values {
                let index = match enum_values.iter().position(|existing| *existing == value) {
                    Some(index) => index,
                    None => {
                        enum_values.push(value);
                        enum_values.len() - 1
                    }
                };
                indexes.push(index as u32);
            }

            enums.push((
                format!("{}Aliases", capitalize(&command.name)),
                indexes,
            ));
            alias_indexes.push((enums.len() - 1) as i32);
        }

        // 1. Enum values.
        writer.write_var_u32(enum_values.len() as u32);
        for value in &enum_values {
            writer.write_string(value);
        }

        // 2. Chained subcommand values (none).
        writer.write_var_u32(0);

        // 3. Postfixes (none).
        writer.write_var_u32(0);

        // 4. Hard enums: alias lists.
        writer.write_var_u32(enums.len() as u32);
        for (name, indexes) in &enums {
            writer.write_string(name);
            writer.write_var_u32(indexes.len() as u32);
            for index in indexes {
                writer.write_u32_le(*index);
            }
        }

        // 5. Chained subcommand data (none).
        writer.write_var_u32(0);

        // 6. Commands.
        writer.write_var_u32(self.commands.len() as u32);
        for (command, alias_index) in self.commands.iter().zip(alias_indexes) {
            writer.write_string(&command.name.to_lowercase());
            writer.write_string(&command.description);
            writer.write_u16_le(0); // flags
            writer.write_string(PERMISSION_ANY);
            writer.write_i32_le(alias_index); // -1 when the command has no aliases
            writer.write_var_u32(0); // chained subcommand indexes

            // A single overload with one optional free-text argument, which is
            // how Axolotl advertises every built-in command.
            writer.write_var_u32(1);
            writer.write_bool(false); // chaining
            writer.write_var_u32(1); // parameter count
            writer.write_string("args");
            writer.write_u32_le(ARG_FLAG_VALID | ARG_TYPE_RAWTEXT);
            writer.write_bool(true); // optional
            writer.write_byte(0); // parameter flags
        }

        // 7. Soft enums (none).
        writer.write_var_u32(0);

        // 8. Enum constraints (none).
        writer.write_var_u32(0);
    }
}

fn capitalize(name: &str) -> String {
    let mut characters = name.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::Reader;

    /// Packet captured from vanilla BDS 1.26.60 (protocol 2225).
    const CAPTURED: &[u8] =
        include_bytes!("../../../../data/bds-capture/available_commands/1.bin");

    /// Parses the payload and returns `(name, alias enum index)` per command.
    /// Panics on any layout mismatch, so it doubles as a format check.
    fn parse(reader: &mut Reader) -> Vec<(String, i32)> {
        for _ in 0..reader.read_var_u32().unwrap() {
            let _ = reader.read_string().unwrap();
        }
        for _ in 0..reader.read_var_u32().unwrap() {
            let _ = reader.read_string().unwrap();
        }
        for _ in 0..reader.read_var_u32().unwrap() {
            let _ = reader.read_string().unwrap();
        }

        for _ in 0..reader.read_var_u32().unwrap() {
            let _ = reader.read_string().unwrap();
            for _ in 0..reader.read_var_u32().unwrap() {
                let _ = reader.read_u32_le().unwrap();
            }
        }

        for _ in 0..reader.read_var_u32().unwrap() {
            let _ = reader.read_string().unwrap();
            for _ in 0..reader.read_var_u32().unwrap() {
                let _ = reader.read_var_u32().unwrap();
                let _ = reader.read_var_u32().unwrap();
            }
        }

        let mut commands = Vec::new();
        for _ in 0..reader.read_var_u32().unwrap() {
            let name = reader.read_string().unwrap();
            let _description = reader.read_string().unwrap();
            let _flags = reader.read_u16_le().unwrap();
            let permission = reader.read_string().unwrap();
            assert!(!permission.is_empty(), "permission must be a string");
            let alias_index = reader.read_i32_le().unwrap();

            for _ in 0..reader.read_var_u32().unwrap() {
                let _ = reader.read_u32_le().unwrap();
            }

            for _ in 0..reader.read_var_u32().unwrap() {
                let _chaining = reader.read_bool().unwrap();
                for _ in 0..reader.read_var_u32().unwrap() {
                    let _parameter = reader.read_string().unwrap();
                    let _type_info = reader.read_u32_le().unwrap();
                    let _optional = reader.read_bool().unwrap();
                    let _flags = reader.read_byte().unwrap();
                }
            }

            commands.push((name, alias_index));
        }

        for _ in 0..reader.read_var_u32().unwrap() {
            let _ = reader.read_string().unwrap();
            for _ in 0..reader.read_var_u32().unwrap() {
                let _ = reader.read_string().unwrap();
            }
        }
        for _ in 0..reader.read_var_u32().unwrap() {
            let _ = reader.read_u32_le().unwrap();
            let _ = reader.read_u32_le().unwrap();
            for _ in 0..reader.read_var_u32().unwrap() {
                let _ = reader.read_byte().unwrap();
            }
        }

        commands
    }

    #[test]
    fn captured_bds_packet_matches_the_encoder_layout() {
        let mut reader = Reader::new(CAPTURED);
        assert_eq!(
            reader.read_var_u32().unwrap(),
            AvailableCommandsPacket::PACKET_ID,
            "capture should start with the packet id"
        );

        let commands = parse(&mut reader);
        assert_eq!(reader.remaining(), 0, "payload must be consumed exactly");
        assert!(commands.len() > 50, "unexpected command count: {}", commands.len());
        for expected in ["teleport", "gamemode", "time", "weather", "say", "kick"] {
            assert!(
                commands.iter().any(|(name, _)| name == expected),
                "captured packet is missing /{expected}"
            );
        }
    }

    #[test]
    fn encoded_commands_roundtrip_through_the_same_layout() {
        let packet = AvailableCommandsPacket {
            commands: vec![
                CommandSpec {
                    name: "tp".to_string(),
                    description: "Teletransporta jugadores".to_string(),
                    aliases: vec!["teleport".to_string()],
                },
                CommandSpec {
                    name: "say".to_string(),
                    description: "Envía un mensaje".to_string(),
                    aliases: Vec::new(),
                },
            ],
        };

        let mut writer = Writer::new();
        packet.encode_payload(&mut writer);
        let bytes = writer.into_bytes();

        let mut reader = Reader::new(&bytes);
        let commands = parse(&mut reader);
        assert_eq!(reader.remaining(), 0);
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].0, "tp");
        assert_eq!(commands[1].0, "say");
        assert!(commands[0].1 >= 0, "/tp should reference its alias enum");
        assert_eq!(commands[1].1, -1, "/say has no aliases");
    }
}
