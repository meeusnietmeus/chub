use std::{path::Path, process::Command};
use crate::config::parser::KdlConfig;

// &'static str → String: entries can now come from either compile-time
// constants or the runtime config file.
struct CommandEntry {
    name: String,
    shorthand: String,
    external: bool,
    command: String,
    background: bool,
}

// Small helper so the literals in `new()` stay readable.
fn entry(name: &str, shorthand: &str, command: &str, external: bool, background: bool) -> CommandEntry {
    CommandEntry {
        name: name.into(),
        shorthand: shorthand.into(),
        command: command.into(),
        external,
        background,
    }
}

pub struct CommandDispatcher {
    commands: Vec<CommandEntry>,
}

impl CommandDispatcher {
    pub fn new() -> Self {
        Self {
            commands: vec![
                entry("shutdown", "sd", "shutdown -h now", false, false),
                entry("reboot",   "rb", "reboot",          false, false),
                entry("update",   "upd", "yay -Syu",       true,  true),
            ],
        }
    }

    /// Reads `commands { … }` from the KDL config at `path` and merges them
    /// with the built-in list. Config entries take precedence over built-ins
    /// with the same name. Duplicate shorthands across *different* names panic.
    pub fn init_from_config(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let config = KdlConfig::from_path(path)?;

        let Some(section) = config.section("commands") else {
            return Ok(()); // no commands block → nothing to do
        };

        for node in section.children() {
            let name = node.name().to_string();
            println!("{}", name);

            let command = node
                .get_str("command")
                .ok_or_else(|| format!("command '{}' is missing the required 'command' field", name))?
                .to_string();

            let shorthand = node.get_str("shorthand").unwrap_or("").to_string();
            let external  = node.get_bool("external").unwrap_or(false);
            let background = node.get_bool("background").unwrap_or(false);

            // Reject duplicate shorthands (but only when they belong to a
            // *different* name — same name means we're overriding a built-in).
            if !shorthand.is_empty() {
                if let Some(conflict) = self
                    .commands
                    .iter()
                    .find(|c| c.shorthand == shorthand && c.name != name)
                {
                    return Err(format!(
                        "shorthand '{}' for '{}' is already used by '{}'",
                        shorthand, name, conflict.name
                    )
                    .into());
                }
            }

            // Config takes precedence: drop any existing entry with the same name.
            self.commands.retain(|c| c.name != name);

            self.commands.push(CommandEntry { name, shorthand, command, external, background });
        }

        Ok(())
    }

    pub fn dispatch(&self, input: &str) {
        let input = input.trim();

        let entry = match self
            .commands
            .iter()
            .find(|c| c.name == input || c.shorthand == input)
        {
            Some(e) => e,
            None => return,
        };

        if entry.external {
            spawn_in_terminal(&entry.command);
        } else {
            spawn_detached(&entry.command);
        }
    }
}

fn spawn_detached(command: &str) {
    let mut parts = command.split_whitespace();
    let Some(program) = parts.next() else { return };
    let args: Vec<&str> = parts.collect();

    if let Err(e) = Command::new(program).args(&args).spawn() {
        eprintln!("failed to spawn '{}': {}", command, e);
    }
}

fn spawn_in_terminal(command: &str) {
    let terminal = std::env::var("TERM")
        .or_else(|_| std::env::var("TERMINAL"))
        .unwrap_or_else(|_| "xdg-terminal-exec".to_string());

    let argv: Vec<&str> = command.split_whitespace().collect();

    let result = Command::new("niri")
        .args(["msg", "action", "spawn", "--", &terminal])
        .args(&argv)
        .spawn();

    if let Err(e) = result {
        eprintln!("failed to open terminal for '{}': {}", command, e);
    }
}
