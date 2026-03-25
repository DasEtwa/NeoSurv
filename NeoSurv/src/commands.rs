use glam::Vec3;

use crate::{
    gameplay::EnemyKind,
    inventory::ItemId,
    world::state::{ChestTier, WorldRuntimeState},
};

#[derive(Debug, Default)]
pub(crate) struct CommandOutcome {
    pub(crate) lines: Vec<String>,
    pub(crate) save_requested: bool,
    pub(crate) load_requested: bool,
}

pub(crate) struct CommandContext<'a> {
    pub(crate) world: &'a mut WorldRuntimeState,
    pub(crate) player_position: Vec3,
    pub(crate) player_forward: Vec3,
}

#[derive(Debug, Default)]
pub(crate) struct CommandRegistry;

impl CommandRegistry {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn execute(&self, input: &str, ctx: &mut CommandContext<'_>) -> CommandOutcome {
        let trimmed = input.trim();
        if !trimmed.starts_with('/') {
            return CommandOutcome {
                lines: vec![format!("LOCAL CHAT: {trimmed}")],
                ..CommandOutcome::default()
            };
        }

        let mut parts = trimmed.split_whitespace();
        let command = parts.next().unwrap_or_default().trim_start_matches('/');

        match command {
            "help" => CommandOutcome {
                lines: vec![
                    "COMMANDS: /HELP /DAY /NIGHT /TIME SET <0..1>".to_string(),
                    "/GIVE <ITEM> <COUNT> /HEAL <AMOUNT> /SPAWN ENEMY <COUNT>".to_string(),
                    "/SPAWN CHEST <COMMON|RARE|EPIC> /SAVE /LOAD".to_string(),
                ],
                ..CommandOutcome::default()
            },
            "day" => {
                ctx.world.time_of_day.set_normalized_time(0.20);
                CommandOutcome {
                    lines: vec!["TIME SET TO DAY".to_string()],
                    ..CommandOutcome::default()
                }
            }
            "night" => {
                ctx.world.time_of_day.set_normalized_time(0.70);
                CommandOutcome {
                    lines: vec!["TIME SET TO NIGHT".to_string()],
                    ..CommandOutcome::default()
                }
            }
            "time" => {
                let subcommand = parts.next().unwrap_or_default();
                let value = parts.next().and_then(|value| value.parse::<f32>().ok());
                match (subcommand, value) {
                    ("set", Some(value)) => {
                        ctx.world.time_of_day.set_normalized_time(value);
                        CommandOutcome {
                            lines: vec![format!("TIME SET TO {:.2}", value.rem_euclid(1.0))],
                            ..CommandOutcome::default()
                        }
                    }
                    _ => CommandOutcome {
                        lines: vec!["USAGE: /TIME SET <0..1>".to_string()],
                        ..CommandOutcome::default()
                    },
                }
            }
            "give" => {
                let item = parts.next().and_then(ItemId::from_token);
                let count = parts
                    .next()
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or(1);
                match item {
                    Some(item_id) => {
                        let mut lines = Vec::new();
                        if let Some(player) = ctx.world.local_player_mut() {
                            let remaining = player.inventory.grant_item(item_id, count);
                            let gained = count.saturating_sub(remaining);
                            lines.push(format!("GAVE {} X{}", item_id.label(), gained));
                            if remaining > 0 {
                                lines.push(format!("NO ROOM FOR {} MORE", remaining));
                            }
                        }
                        CommandOutcome {
                            lines,
                            ..CommandOutcome::default()
                        }
                    }
                    None => CommandOutcome {
                        lines: vec![
                            "USAGE: /GIVE <SIDEARM|LAUNCHER|MEDKIT|GRENADE> <COUNT>".to_string(),
                        ],
                        ..CommandOutcome::default()
                    },
                }
            }
            "heal" => {
                let amount = parts
                    .next()
                    .and_then(|value| value.parse::<i32>().ok())
                    .unwrap_or(35);
                if let Some(player) = ctx.world.local_player_mut() {
                    player.heal(amount);
                    return CommandOutcome {
                        lines: vec![format!("HEALED TO {}", player.health)],
                        ..CommandOutcome::default()
                    };
                }
                CommandOutcome::default()
            }
            "spawn" => {
                let kind = parts.next().unwrap_or_default();
                match kind {
                    "enemy" => {
                        let count = parts
                            .next()
                            .and_then(|value| value.parse::<usize>().ok())
                            .unwrap_or(1);
                        for index in 0..count {
                            let id = ctx.world.alloc_id();
                            let offset =
                                ctx.player_forward.normalize_or_zero() * (6.0 + index as f32 * 1.5);
                            let position = ctx.player_position + offset;
                            ctx.world.enemies.spawn_enemy(
                                id,
                                EnemyKind::MeleeHunter,
                                Vec3::new(position.x, position.y - 1.4, position.z),
                                None,
                            );
                        }
                        CommandOutcome {
                            lines: vec![format!("SPAWNED {count} ENEMIES")],
                            ..CommandOutcome::default()
                        }
                    }
                    "chest" => {
                        let tier = match parts
                            .next()
                            .unwrap_or_default()
                            .to_ascii_lowercase()
                            .as_str()
                        {
                            "common" => Some(ChestTier::Common),
                            "rare" => Some(ChestTier::Rare),
                            "epic" => Some(ChestTier::Epic),
                            _ => None,
                        };
                        match tier {
                            Some(tier) => {
                                let position = (ctx.player_position
                                    + ctx.player_forward.normalize_or_zero() * 4.0)
                                    .round()
                                    .as_ivec3();
                                ctx.world.spawn_debug_chest(position, tier);
                                CommandOutcome {
                                    lines: vec![format!("SPAWNED {} CHEST", tier.label())],
                                    ..CommandOutcome::default()
                                }
                            }
                            None => CommandOutcome {
                                lines: vec!["USAGE: /SPAWN CHEST <COMMON|RARE|EPIC>".to_string()],
                                ..CommandOutcome::default()
                            },
                        }
                    }
                    _ => CommandOutcome {
                        lines: vec![
                            "USAGE: /SPAWN ENEMY <COUNT> OR /SPAWN CHEST <TIER>".to_string(),
                        ],
                        ..CommandOutcome::default()
                    },
                }
            }
            "save" => CommandOutcome {
                lines: vec!["WORLD SAVE REQUESTED".to_string()],
                save_requested: true,
                ..CommandOutcome::default()
            },
            "load" => CommandOutcome {
                lines: vec!["WORLD LOAD REQUESTED".to_string()],
                load_requested: true,
                ..CommandOutcome::default()
            },
            _ => CommandOutcome {
                lines: vec![format!(
                    "UNKNOWN COMMAND: /{}",
                    command.to_ascii_uppercase()
                )],
                ..CommandOutcome::default()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::state::WorldRuntimeState;

    #[test]
    fn time_command_updates_time_state() {
        let mut world = WorldRuntimeState::new_singleplayer(7);
        let registry = CommandRegistry::new();
        let mut context = CommandContext {
            world: &mut world,
            player_position: Vec3::ZERO,
            player_forward: Vec3::Z,
        };

        let outcome = registry.execute("/time set 0.75", &mut context);
        assert_eq!(outcome.lines[0], "TIME SET TO 0.75");
        assert!(context.world.time_of_day.is_night());
    }
}
