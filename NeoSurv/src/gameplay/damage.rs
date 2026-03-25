#[derive(Debug, Clone, Copy)]
pub(super) struct DamageEvent {
    pub(super) amount: i32,
}

#[derive(Debug, Clone, Copy)]
pub(super) enum DamageResolution {
    Survived { remaining_hp: i32 },
    Eliminated,
}

pub(super) fn resolve_damage(current_hp: i32, event: DamageEvent) -> DamageResolution {
    let remaining_hp = current_hp - event.amount.max(0);

    if remaining_hp <= 0 {
        DamageResolution::Eliminated
    } else {
        DamageResolution::Survived { remaining_hp }
    }
}
