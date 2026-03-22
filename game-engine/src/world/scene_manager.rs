use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use legion::World;

use crate::world::{save_load, scene::Scene, scene_sync};

#[derive(Debug, Clone)]
pub(crate) struct SceneManager {
    scenes_dir: PathBuf,
    active_scene_id: String,
    current_scene: Scene,
}

impl SceneManager {
    pub(crate) const DEFAULT_SCENES_DIR: &str = "assets/scenes";
    pub(crate) const DEFAULT_SCENE_ID: &str = "default";
    pub(crate) const QUICK_SCENE_ID: &str = "quick";

    pub(crate) fn new(current_scene: Scene) -> Self {
        Self {
            scenes_dir: PathBuf::from(Self::DEFAULT_SCENES_DIR),
            active_scene_id: Self::DEFAULT_SCENE_ID.to_owned(),
            current_scene,
        }
    }

    pub(crate) fn active_scene_id(&self) -> &str {
        &self.active_scene_id
    }

    pub(crate) fn set_active_scene_id(&mut self, scene_id: impl Into<String>) -> Result<()> {
        let scene_id = scene_id.into();
        Self::validate_scene_id(&scene_id)?;
        self.active_scene_id = scene_id;
        Ok(())
    }

    pub(crate) fn current_scene(&self) -> &Scene {
        &self.current_scene
    }

    pub(crate) fn save_active_scene(&self) -> Result<PathBuf> {
        self.save_scene_slot(&self.active_scene_id)
    }

    pub(crate) fn load_active_scene(&mut self) -> Result<PathBuf> {
        let active_scene_id = self.active_scene_id.clone();
        self.load_scene_slot(&active_scene_id)
    }

    pub(crate) fn save_scene_slot(&self, scene_id: &str) -> Result<PathBuf> {
        let path = self.scene_path(scene_id)?;

        save_load::save_scene_to_path(&path, &self.current_scene).with_context(|| {
            format!(
                "failed to save scene '{}' to slot '{}'",
                self.current_scene.name, scene_id
            )
        })?;

        Ok(path)
    }

    pub(crate) fn load_scene_slot(&mut self, scene_id: &str) -> Result<PathBuf> {
        let path = self.scene_path(scene_id)?;

        let loaded_scene = save_load::load_scene_from_path(&path)
            .with_context(|| format!("failed to load scene slot '{}'", scene_id))?;

        self.current_scene = loaded_scene;

        Ok(path)
    }

    pub(crate) fn save_world_to_active_scene(&mut self, world: &World) -> Result<PathBuf> {
        let active_scene_id = self.active_scene_id.clone();
        self.save_world_to_slot(world, &active_scene_id)
    }

    pub(crate) fn load_active_scene_into_world(&mut self, world: &mut World) -> Result<PathBuf> {
        let active_scene_id = self.active_scene_id.clone();
        self.load_scene_slot_into_world(world, &active_scene_id)
    }

    pub(crate) fn apply_current_scene_to_world(&self, world: &mut World) -> Result<()> {
        scene_sync::replace_world_from_scene(world, &self.current_scene).with_context(|| {
            format!(
                "failed to apply scene '{}' into ECS world",
                self.current_scene.name
            )
        })
    }

    pub(crate) fn save_world_to_slot(&mut self, world: &World, scene_id: &str) -> Result<PathBuf> {
        Self::validate_scene_id(scene_id)?;

        self.current_scene = scene_sync::scene_from_world(world, scene_id);

        self.save_scene_slot(scene_id)
    }

    pub(crate) fn load_scene_slot_into_world(
        &mut self,
        world: &mut World,
        scene_id: &str,
    ) -> Result<PathBuf> {
        let path = self.load_scene_slot(scene_id)?;

        self.apply_current_scene_to_world(world).with_context(|| {
            format!(
                "failed to apply loaded scene '{}' (slot '{}') into ECS world",
                self.current_scene.name, scene_id
            )
        })?;

        Ok(path)
    }

    fn scene_path(&self, scene_id: &str) -> Result<PathBuf> {
        Self::validate_scene_id(scene_id)?;
        Ok(self.scenes_dir.join(format!("{}.ron", scene_id)))
    }

    fn validate_scene_id(scene_id: &str) -> Result<()> {
        if scene_id.is_empty() {
            bail!("scene slot id must not be empty");
        }

        if !scene_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
        {
            bail!(
                "invalid scene slot id '{}': only [A-Za-z0-9_-] are allowed",
                scene_id
            );
        }

        Ok(())
    }
}
