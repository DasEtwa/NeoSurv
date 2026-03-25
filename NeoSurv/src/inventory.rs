use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) enum ItemId {
    Sidearm,
    Launcher,
    Medkit,
    Grenade,
}

impl ItemId {
    pub(crate) fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_lowercase().as_str() {
            "sidearm" | "pistol" => Some(Self::Sidearm),
            "launcher" | "rocket" => Some(Self::Launcher),
            "medkit" | "heal" | "health" => Some(Self::Medkit),
            "grenade" | "frag" => Some(Self::Grenade),
            _ => None,
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Sidearm => "SIDEARM",
            Self::Launcher => "LAUNCHER",
            Self::Medkit => "MEDKIT",
            Self::Grenade => "GRENADE",
        }
    }

    pub(crate) fn max_stack(self) -> u32 {
        match self {
            Self::Sidearm | Self::Launcher => 1,
            Self::Medkit => 6,
            Self::Grenade => 8,
        }
    }

    pub(crate) fn preferred_slot(self) -> usize {
        match self {
            Self::Sidearm => 0,
            Self::Launcher => 1,
            Self::Medkit => 2,
            Self::Grenade => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum HotbarSlotKind {
    WeaponPrimary,
    WeaponSecondary,
    Heal,
    Throwable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ItemStack {
    pub(crate) item_id: ItemId,
    pub(crate) count: u32,
}

impl ItemStack {
    pub(crate) fn new(item_id: ItemId, count: u32) -> Self {
        Self {
            item_id,
            count: count.max(1).min(item_id.max_stack()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct HotbarSlot {
    pub(crate) kind: HotbarSlotKind,
    pub(crate) stack: Option<ItemStack>,
}

impl HotbarSlot {
    const fn with_stack(kind: HotbarSlotKind, stack: Option<ItemStack>) -> Self {
        Self { kind, stack }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InventoryState {
    pub(crate) slots: [HotbarSlot; 4],
    pub(crate) selected_weapon_slot: usize,
}

impl Default for InventoryState {
    fn default() -> Self {
        Self::new_default_loadout()
    }
}

impl InventoryState {
    pub(crate) fn new_default_loadout() -> Self {
        Self {
            slots: [
                HotbarSlot::with_stack(
                    HotbarSlotKind::WeaponPrimary,
                    Some(ItemStack::new(ItemId::Sidearm, 1)),
                ),
                HotbarSlot::with_stack(
                    HotbarSlotKind::WeaponSecondary,
                    Some(ItemStack::new(ItemId::Launcher, 1)),
                ),
                HotbarSlot::with_stack(
                    HotbarSlotKind::Heal,
                    Some(ItemStack::new(ItemId::Medkit, 2)),
                ),
                HotbarSlot::with_stack(
                    HotbarSlotKind::Throwable,
                    Some(ItemStack::new(ItemId::Grenade, 3)),
                ),
            ],
            selected_weapon_slot: 0,
        }
    }

    pub(crate) fn select_weapon_slot(&mut self, slot: usize) {
        self.selected_weapon_slot = slot.min(1);
    }

    pub(crate) fn selected_weapon(&self) -> Option<ItemId> {
        self.slots
            .get(self.selected_weapon_slot)
            .and_then(|slot| slot.stack)
            .map(|stack| stack.item_id)
    }

    pub(crate) fn grant_item(&mut self, item_id: ItemId, count: u32) -> u32 {
        let mut remaining = count.max(1);
        let preferred_slot = item_id.preferred_slot();

        if let Some(slot) = self.slots.get_mut(preferred_slot) {
            match slot.stack {
                Some(mut stack) if stack.item_id == item_id => {
                    let free = item_id.max_stack().saturating_sub(stack.count);
                    let added = free.min(remaining);
                    stack.count += added;
                    slot.stack = Some(stack);
                    remaining -= added;
                }
                None => {
                    let added = item_id.max_stack().min(remaining);
                    slot.stack = Some(ItemStack::new(item_id, added));
                    remaining -= added;
                }
                _ => {}
            }
        }

        remaining
    }

    pub(crate) fn consume_heal(&mut self) -> bool {
        self.consume_from_slot(2, ItemId::Medkit)
    }

    pub(crate) fn consume_throwable(&mut self) -> bool {
        self.consume_from_slot(3, ItemId::Grenade)
    }

    fn consume_from_slot(&mut self, index: usize, expected_item: ItemId) -> bool {
        let Some(slot) = self.slots.get_mut(index) else {
            return false;
        };
        let Some(mut stack) = slot.stack else {
            return false;
        };
        if stack.item_id != expected_item || stack.count == 0 {
            return false;
        }

        stack.count -= 1;
        slot.stack = (stack.count > 0).then_some(stack);
        true
    }
}

pub(crate) fn clamp_health(health: i32, max_health: i32) -> i32 {
    health.clamp(0, max_health.max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_item_stacks_into_matching_slot() {
        let mut inventory = InventoryState::new_default_loadout();
        let remaining = inventory.grant_item(ItemId::Grenade, 2);
        assert_eq!(remaining, 0);
        assert_eq!(inventory.slots[3].stack.unwrap().count, 5);
    }

    #[test]
    fn consumables_are_removed_from_hotbar() {
        let mut inventory = InventoryState::new_default_loadout();
        assert!(inventory.consume_heal());
        assert_eq!(inventory.slots[2].stack.unwrap().count, 1);
        assert!(inventory.consume_throwable());
        assert_eq!(inventory.slots[3].stack.unwrap().count, 2);
    }
}
