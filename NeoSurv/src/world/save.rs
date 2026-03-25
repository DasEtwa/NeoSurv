use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{player::SavedPlayerPose, world::state::WorldRuntimeState};

const SAVE_DIR_NAME: &str = "saves";
const WORLD_DIR_NAME: &str = "worlds";
const INDEX_FILE_NAME: &str = "index.toml";
const SAVE_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SavedWorld {
    pub(crate) version: u32,
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) seed: u32,
    pub(crate) created_unix_secs: u64,
    pub(crate) last_played_unix_secs: u64,
    pub(crate) runtime_state: WorldRuntimeState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct WorldSummary {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) seed: u32,
    pub(crate) created_unix_secs: u64,
    pub(crate) last_played_unix_secs: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SaveIndex {
    selected_world_id: Option<String>,
    worlds: Vec<WorldSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacySavedWorld {
    version: u32,
    id: String,
    name: String,
    seed: u32,
    created_unix_secs: u64,
    last_played_unix_secs: u64,
    player_pose: Option<SavedPlayerPose>,
}

#[derive(Debug)]
pub(crate) struct WorldSaveManager {
    root_dir: PathBuf,
    index_path: PathBuf,
    worlds_dir: PathBuf,
    index: SaveIndex,
}

impl WorldSaveManager {
    pub(crate) fn load_or_default(root: impl AsRef<Path>) -> Self {
        let root_dir = root.as_ref().join(SAVE_DIR_NAME);
        let worlds_dir = root_dir.join(WORLD_DIR_NAME);
        let index_path = root_dir.join(INDEX_FILE_NAME);

        let mut manager = Self {
            root_dir,
            index_path,
            worlds_dir,
            index: SaveIndex::default(),
        };

        if let Err(err) = manager.load_index() {
            tracing::warn!(?err, "failed to load world save index, rebuilding defaults");
        }

        if let Err(err) = manager.ensure_default_world() {
            tracing::warn!(?err, "failed to initialize default world save");
        }

        manager
    }

    pub(crate) fn selected_world(&self) -> Option<&WorldSummary> {
        let selected_id = self.index.selected_world_id.as_ref()?;
        self.index
            .worlds
            .iter()
            .find(|world| &world.id == selected_id)
    }

    pub(crate) fn selected_world_name(&self) -> String {
        self.selected_world()
            .map(|world| world.name.clone())
            .unwrap_or_else(|| "WORLD".to_string())
    }

    pub(crate) fn load_selected_world(&self) -> Option<SavedWorld> {
        let selected = self.selected_world()?;
        self.load_world_file(&selected.id).ok()
    }

    pub(crate) fn select_next_world(&mut self) {
        if self.index.worlds.is_empty() {
            return;
        }

        let current_index =
            self.selected_world_position().unwrap_or(0).wrapping_add(1) % self.index.worlds.len();
        self.index.selected_world_id = Some(self.index.worlds[current_index].id.clone());
        self.persist_index();
    }

    pub(crate) fn select_previous_world(&mut self) {
        if self.index.worlds.is_empty() {
            return;
        }

        let current_index = self.selected_world_position().unwrap_or(0);
        let previous_index = if current_index == 0 {
            self.index.worlds.len() - 1
        } else {
            current_index - 1
        };
        self.index.selected_world_id = Some(self.index.worlds[previous_index].id.clone());
        self.persist_index();
    }

    pub(crate) fn create_world(&mut self) -> Option<SavedWorld> {
        let world_number = self.index.worlds.len() + 1;
        let now = now_unix_secs();
        let id = format!("world-{}-{now}", world_number);
        let name = format!("WORLD {world_number}");
        let seed = seed_from_time(now, world_number as u64);
        let world = SavedWorld {
            version: SAVE_FILE_VERSION,
            id: id.clone(),
            name: name.clone(),
            seed,
            created_unix_secs: now,
            last_played_unix_secs: now,
            runtime_state: WorldRuntimeState::new_singleplayer(seed),
        };

        if let Err(err) = self.write_world_file(&world) {
            tracing::warn!(?err, world = world.name, "failed to write new world file");
            return None;
        }

        self.index.worlds.push(WorldSummary {
            id: id.clone(),
            name,
            seed,
            created_unix_secs: now,
            last_played_unix_secs: now,
        });
        self.index.selected_world_id = Some(id);
        self.persist_index();

        Some(world)
    }

    pub(crate) fn save_selected_world(&mut self, runtime_state: &WorldRuntimeState) {
        let Some(mut world) = self.load_selected_world() else {
            return;
        };

        world.runtime_state = runtime_state.clone();
        world.last_played_unix_secs = now_unix_secs();

        if let Err(err) = self.write_world_file(&world) {
            tracing::warn!(?err, world = world.name, "failed to save world file");
            return;
        }

        if let Some(summary) = self
            .index
            .worlds
            .iter_mut()
            .find(|summary| summary.id == world.id)
        {
            summary.last_played_unix_secs = world.last_played_unix_secs;
        }
        self.persist_index();
    }

    fn selected_world_position(&self) -> Option<usize> {
        let selected_id = self.index.selected_world_id.as_ref()?;
        self.index
            .worlds
            .iter()
            .position(|world| &world.id == selected_id)
    }

    fn load_index(&mut self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.worlds_dir)?;

        if !self.index_path.exists() {
            return Ok(());
        }

        let text = fs::read_to_string(&self.index_path)?;
        self.index = toml::from_str(&text)?;
        Ok(())
    }

    fn ensure_default_world(&mut self) -> anyhow::Result<()> {
        fs::create_dir_all(&self.root_dir)?;
        fs::create_dir_all(&self.worlds_dir)?;

        if !self.index.worlds.is_empty() {
            if self.index.selected_world_id.is_none() {
                self.index.selected_world_id =
                    self.index.worlds.first().map(|world| world.id.clone());
                self.persist_index();
            }
            return Ok(());
        }

        let default_world = self
            .create_world()
            .ok_or_else(|| anyhow::anyhow!("failed to create default world"))?;
        tracing::info!(
            world = default_world.name,
            seed = default_world.seed,
            "default world save created"
        );
        Ok(())
    }

    fn world_file_path(&self, world_id: &str) -> PathBuf {
        self.worlds_dir.join(format!("{world_id}.toml"))
    }

    fn load_world_file(&self, world_id: &str) -> anyhow::Result<SavedWorld> {
        let text = fs::read_to_string(self.world_file_path(world_id))?;
        if let Ok(saved) = toml::from_str(&text) {
            return Ok(saved);
        }

        let legacy: LegacySavedWorld = toml::from_str(&text)?;
        let mut runtime_state = WorldRuntimeState::new_singleplayer(legacy.seed);
        if let Some(pose) = legacy.player_pose {
            runtime_state.sync_local_player_pose(pose);
        }

        Ok(SavedWorld {
            version: legacy.version,
            id: legacy.id,
            name: legacy.name,
            seed: legacy.seed,
            created_unix_secs: legacy.created_unix_secs,
            last_played_unix_secs: legacy.last_played_unix_secs,
            runtime_state,
        })
    }

    fn write_world_file(&self, world: &SavedWorld) -> anyhow::Result<()> {
        fs::create_dir_all(&self.worlds_dir)?;
        fs::write(
            self.world_file_path(&world.id),
            toml::to_string_pretty(world)?,
        )?;
        Ok(())
    }

    fn persist_index(&self) {
        if let Err(err) = fs::create_dir_all(&self.root_dir).and_then(|_| {
            fs::write(
                &self.index_path,
                toml::to_string_pretty(&self.index).unwrap_or_default(),
            )
        }) {
            tracing::warn!(?err, "failed to persist world save index");
        }
    }
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn seed_from_time(now: u64, salt: u64) -> u32 {
    let mixed = now ^ (salt << 21) ^ 0xC0FF_EE42u64;
    (mixed as u32).rotate_left(13) ^ ((mixed >> 17) as u32)
}
