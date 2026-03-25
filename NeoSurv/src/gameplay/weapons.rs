#[derive(Debug, Clone, Copy)]
pub(crate) enum WeaponFireMode {
    Hitscan {
        range: f32,
    },
    Projectile {
        speed: f32,
        gravity: f32,
        max_lifetime: f32,
        radius: f32,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct WeaponDefinition {
    pub(crate) id: &'static str,
    pub(crate) model_asset_path: &'static str,
    pub(crate) shot_damage: i32,
    pub(crate) muzzle_flash_time: f32,
    pub(crate) recoil_time: f32,
    pub(crate) viewmodel_distance: f32,
    pub(crate) viewmodel_right_offset: f32,
    pub(crate) viewmodel_down_offset: f32,
    pub(crate) viewmodel_scale: f32,
    pub(crate) viewmodel_recoil_distance: f32,
    pub(crate) fire_mode: WeaponFireMode,
}

impl WeaponDefinition {
    pub(crate) const fn sidearm() -> Self {
        Self {
            id: "sidearm",
            model_asset_path: "assets/models/pistol_1/Pistol_1.obj",
            shot_damage: 34,
            muzzle_flash_time: 0.07,
            recoil_time: 0.10,
            viewmodel_distance: 0.36,
            viewmodel_right_offset: 0.12,
            viewmodel_down_offset: 0.18,
            viewmodel_scale: 0.52,
            viewmodel_recoil_distance: 0.06,
            fire_mode: WeaponFireMode::Hitscan { range: 96.0 },
        }
    }

    pub(crate) const fn launcher() -> Self {
        Self {
            id: "launcher",
            model_asset_path: "assets/models/pistol_1/Pistol_1.obj",
            shot_damage: 45,
            muzzle_flash_time: 0.10,
            recoil_time: 0.16,
            viewmodel_distance: 0.34,
            viewmodel_right_offset: 0.10,
            viewmodel_down_offset: 0.16,
            viewmodel_scale: 0.56,
            viewmodel_recoil_distance: 0.08,
            fire_mode: WeaponFireMode::Projectile {
                speed: 24.0,
                gravity: 4.5,
                max_lifetime: 4.0,
                radius: 0.18,
            },
        }
    }

    pub(crate) const fn grenade() -> Self {
        Self {
            id: "grenade",
            model_asset_path: "assets/models/pistol_1/Pistol_1.obj",
            shot_damage: 70,
            muzzle_flash_time: 0.0,
            recoil_time: 0.0,
            viewmodel_distance: 0.30,
            viewmodel_right_offset: 0.10,
            viewmodel_down_offset: 0.18,
            viewmodel_scale: 0.40,
            viewmodel_recoil_distance: 0.0,
            fire_mode: WeaponFireMode::Projectile {
                speed: 16.0,
                gravity: 9.0,
                max_lifetime: 3.2,
                radius: 0.24,
            },
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct WeaponEffects {
    shot_flash_timer: f32,
    shot_recoil_timer: f32,
}

impl WeaponEffects {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn tick(&mut self, dt_seconds: f32) {
        self.shot_flash_timer = (self.shot_flash_timer - dt_seconds).max(0.0);
        self.shot_recoil_timer = (self.shot_recoil_timer - dt_seconds).max(0.0);
    }

    pub(crate) fn register_shot(&mut self, weapon: WeaponDefinition) {
        self.shot_flash_timer = weapon.muzzle_flash_time;
        self.shot_recoil_timer = weapon.recoil_time;
    }

    pub(crate) fn recoil_ratio(&self, weapon: WeaponDefinition) -> f32 {
        (self.shot_recoil_timer / weapon.recoil_time.max(f32::EPSILON)).clamp(0.0, 1.0)
    }

    pub(crate) fn muzzle_flash_active(&self) -> bool {
        self.shot_flash_timer > 0.0
    }
}
