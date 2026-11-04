//! In-game chat command handler for PocketMine-Rust

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    SetGameMode(i32),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandOutput {
    pub action: CommandAction,
    pub message: String,
}

pub fn handle_command(cmd_line: &str) -> Option<CommandOutput> {
    let trimmed = cmd_line.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    let without_slash = &trimmed[1..];
    let parts: Vec<&str> = without_slash.split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    let cmd = parts[0].to_lowercase();
    let args = &parts[1..];

    match cmd.as_str() {
        "gmc" => Some(CommandOutput {
            action: CommandAction::SetGameMode(1),
            message: "§aTu modo de juego se ha cambiado a Creativo.".to_string(),
        }),
        "gms" => Some(CommandOutput {
            action: CommandAction::SetGameMode(0),
            message: "§aTu modo de juego se ha cambiado a Supervivencia.".to_string(),
        }),
        "gamemode" | "gm" => {
            if args.is_empty() {
                Some(CommandOutput {
                    action: CommandAction::None,
                    message: "§cUso: /gamemode <creative|survival|0|1>".to_string(),
                })
            } else {
                match args[0].to_lowercase().as_str() {
                    "c" | "creative" | "1" => Some(CommandOutput {
                        action: CommandAction::SetGameMode(1),
                        message: "§aTu modo de juego se ha cambiado a Creativo.".to_string(),
                    }),
                    "s" | "survival" | "0" => Some(CommandOutput {
                        action: CommandAction::SetGameMode(0),
                        message: "§aTu modo de juego se ha cambiado a Supervivencia.".to_string(),
                    }),
                    "a" | "adventure" | "2" => Some(CommandOutput {
                        action: CommandAction::SetGameMode(2),
                        message: "§aTu modo de juego se ha cambiado a Aventura.".to_string(),
                    }),
                    "sp" | "spectator" | "3" => Some(CommandOutput {
                        action: CommandAction::SetGameMode(3),
                        message: "§aTu modo de juego se ha cambiado a Espectador.".to_string(),
                    }),
                    other => Some(CommandOutput {
                        action: CommandAction::None,
                        message: format!("§cModo de juego desconocido: '{}'. Usa creative o survival.", other),
                    }),
                }
            }
        }
        "help" => Some(CommandOutput {
            action: CommandAction::None,
            message: "§bComandos disponibles:\n§7/gamemode <creative|survival>\n§7/gmc, /gms\n§7/help".to_string(),
        }),
        other => Some(CommandOutput {
            action: CommandAction::None,
            message: format!("§cComando desconocido '/{}'. Usa /help para ver la lista.", other),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_command_gamemode() {
        let res = handle_command("/gamemode creative").unwrap();
        assert_eq!(res.action, CommandAction::SetGameMode(1));

        let res = handle_command("/gmc").unwrap();
        assert_eq!(res.action, CommandAction::SetGameMode(1));

        let res = handle_command("/gamemode 0").unwrap();
        assert_eq!(res.action, CommandAction::SetGameMode(0));

        let res = handle_command("/gms").unwrap();
        assert_eq!(res.action, CommandAction::SetGameMode(0));

        let non_cmd = handle_command("hola mundo");
        assert!(non_cmd.is_none());
    }
}
