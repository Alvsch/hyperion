use std::cmp::min;

use flecs_ecs::{core::World, macros::Component, prelude::Module};
use roaring::RoaringBitmap;
use valence_protocol::{ItemKind, ItemStack};

pub mod action;
pub mod parser;

pub type PlayerInventory = Inventory<46>;

/// Placeholder; this will be added later.
#[derive(Component, Debug)]
pub struct Inventory<const T: usize> {
    slots: [ItemStack; T],
    hand_slot: u16,
    pub updated_since_last_tick: RoaringBitmap, // todo: maybe make this private
    pub hand_slot_updated_since_last_tick: bool, // todo: maybe make this private
}

#[derive(Debug)]
pub struct AddItemResult {
    pub remaining: Option<ItemStack>,
}

impl<const T: usize> Default for Inventory<T> {
    fn default() -> Self {
        Self {
            slots: [ItemStack::EMPTY; T],
            hand_slot: 0,
            updated_since_last_tick: RoaringBitmap::new(),
            hand_slot_updated_since_last_tick: false,
        }
    }
}

use hyperion_crafting::{Crafting2x2, CraftingRegistry};
use snafu::prelude::*;

#[derive(Debug, Snafu)]
pub enum InventoryAccessError {
    #[snafu(display("Invalid slot index: {index}"))]
    InvalidSlot { index: u16 },
}

enum TryAddSlot {
    Complete,
    Partial,
    Skipped,
}

const HAND_START_SLOT: u16 = 36;

impl<const N: usize> Inventory<N> {
    pub fn set(&mut self, index: u16, stack: ItemStack) -> Result<(), InventoryAccessError> {
        let item = self.get_mut(index)?;
        *item = stack;
        self.updated_since_last_tick.insert(u32::from(index));
        Ok(())
    }

    pub fn items(&self) -> impl Iterator<Item = (u16, &ItemStack)> + '_ {
        self.slots.iter().enumerate().filter_map(|(idx, item)| {
            if item.is_empty() {
                None
            } else {
                Some((u16::try_from(idx).unwrap(), item))
            }
        })
    }

    #[must_use]
    pub const fn slots(&self) -> &[ItemStack; N] {
        &self.slots
    }

    pub fn clear(&mut self) {
        for (idx, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_empty() {
                continue;
            }
            *slot = ItemStack::EMPTY;
            self.updated_since_last_tick
                .insert(u32::try_from(idx).unwrap());
        }
    }

    pub fn set_cursor(&mut self, index: u16) {
        if self.hand_slot == index {
            return;
        }

        self.hand_slot = index;
        self.hand_slot_updated_since_last_tick = true;
    }

    #[must_use]
    pub fn get_cursor(&self) -> &ItemStack {
        self.get_hand_slot(self.hand_slot).unwrap()
    }

    #[must_use]
    pub const fn get_cursor_index(&self) -> u16 {
        self.hand_slot + HAND_START_SLOT
    }

    pub fn get_cursor_mut(&mut self) -> &mut ItemStack {
        self.get_hand_slot_mut(self.hand_slot).unwrap()
    }

    pub fn take_one_held(&mut self) -> ItemStack {
        // decrement the held item
        let held_item = self.get_cursor_mut();

        if held_item.is_empty() {
            return ItemStack::EMPTY;
        }

        held_item.count -= 1;

        ItemStack::new(held_item.item, 1, held_item.nbt.clone())
    }

    pub fn get(&self, index: u16) -> Result<&ItemStack, InventoryAccessError> {
        self.slots
            .get(usize::from(index))
            .ok_or(InventoryAccessError::InvalidSlot { index })
    }

    pub fn get_mut(&mut self, index: u16) -> Result<&mut ItemStack, InventoryAccessError> {
        let Some(slot) = self.slots.get_mut(index as usize) else {
            return Err(InventoryAccessError::InvalidSlot { index });
        };

        // assume that the slot is updated
        self.updated_since_last_tick.insert(u32::from(index));

        Ok(slot)
    }

    pub fn swap(&mut self, index_a: u16, index_b: u16) {
        let index_a = usize::from(index_a);
        let index_b = usize::from(index_b);

        self.slots.swap(index_a, index_b);
    }

    pub fn get_hand_slot(&self, idx: u16) -> Result<&ItemStack, InventoryAccessError> {
        const HAND_END_SLOT: u16 = 45;

        let idx = idx + HAND_START_SLOT;

        if idx >= HAND_END_SLOT {
            return Err(InventoryAccessError::InvalidSlot { index: idx });
        }

        self.get(idx)
    }

    pub fn get_hand_slot_mut(&mut self, idx: u16) -> Result<&mut ItemStack, InventoryAccessError> {
        const HAND_START_SLOT: u16 = 36;
        const HAND_END_SLOT: u16 = 45;

        let idx = idx + HAND_START_SLOT;

        if idx >= HAND_END_SLOT {
            return Err(InventoryAccessError::InvalidSlot { index: idx });
        }

        self.get_mut(idx)
    }

    /// Returns remaining [`ItemStack`] if not all of the item was added to the slot
    fn try_add_to_slot(
        &mut self,
        slot: u16,
        to_add: &mut ItemStack,
        can_add_to_empty: bool,
    ) -> Result<TryAddSlot, InventoryAccessError> {
        let max_stack_size: i8 = to_add.item.max_stack();

        let existing_stack = self.get_mut(slot)?;

        if existing_stack.is_empty() {
            return if can_add_to_empty {
                let new_count = min(to_add.count, max_stack_size);
                *existing_stack = to_add.clone().with_count(new_count);
                to_add.count -= new_count;
                self.updated_since_last_tick.insert(u32::from(slot));
                return if to_add.count > 0 {
                    Ok(TryAddSlot::Partial)
                } else {
                    Ok(TryAddSlot::Complete)
                };
            } else {
                Ok(TryAddSlot::Skipped)
            };
        }

        let stackable = existing_stack.item == to_add.item && existing_stack.nbt == to_add.nbt;

        if stackable && existing_stack.count < max_stack_size {
            let space_left = max_stack_size - existing_stack.count;

            return if to_add.count <= space_left {
                existing_stack.count += to_add.count;
                *to_add = ItemStack::EMPTY;
                self.updated_since_last_tick.insert(u32::from(slot));
                Ok(TryAddSlot::Complete)
            } else {
                existing_stack.count = max_stack_size;
                to_add.count -= space_left;
                self.updated_since_last_tick.insert(u32::from(slot));
                Ok(TryAddSlot::Partial)
            };
        }

        Ok(TryAddSlot::Skipped)
    }
}

impl PlayerInventory {
    pub const BOOTS_SLOT: u16 = 8;
    pub const CHESTPLATE_SLOT: u16 = 6;
    pub const HELMET_SLOT: u16 = 5;
    pub const HOTBAR_START_SLOT: u16 = 36;
    pub const LEGGINGS_SLOT: u16 = 7;
    pub const OFFHAND_SLOT: u16 = OFFHAND_SLOT;

    #[must_use]
    pub fn crafting_result(&self, registry: &CraftingRegistry) -> ItemStack {
        let indices = core::array::from_fn::<u16, 4, _>(|i| (u16::try_from(i).unwrap() + 1));

        let mut min_count = i8::MAX;

        let items: Crafting2x2 = indices.map(|idx| {
            let stack = self.get(idx).unwrap();

            if stack.is_empty() {
                return ItemKind::Air;
            }

            min_count = min_count.min(stack.count);
            stack.item
        });

        let result = registry
            .get_result_2x2(items)
            .cloned()
            .unwrap_or(ItemStack::EMPTY);

        // if result.is_empty() {
        //     return ItemStack::EMPTY;
        // }
        //
        // let new_count = 64.min(min_count as i32 * result.count as i32);
        //
        // result.count = new_count as i8;

        result
    }

    pub fn set_hotbar(&mut self, idx: u16, stack: ItemStack) {
        const HAND_END_SLOT: u16 = 45;

        let idx = idx + HAND_START_SLOT;

        if idx >= HAND_END_SLOT {
            return;
        }

        self.set(idx, stack).unwrap();
    }

    pub fn set_offhand(&mut self, stack: ItemStack) {
        self.set(Self::OFFHAND_SLOT, stack).unwrap();
    }

    pub fn set_helmet(&mut self, stack: ItemStack) {
        self.set(Self::HELMET_SLOT, stack).unwrap();
    }

    pub fn set_chestplate(&mut self, stack: ItemStack) {
        self.set(Self::CHESTPLATE_SLOT, stack).unwrap();
    }

    pub fn set_leggings(&mut self, stack: ItemStack) {
        self.set(Self::LEGGINGS_SLOT, stack).unwrap();
    }

    pub fn set_boots(&mut self, stack: ItemStack) {
        self.set(Self::BOOTS_SLOT, stack).unwrap();
    }

    #[must_use]
    pub fn get_helmet(&self) -> &ItemStack {
        self.get(Self::HELMET_SLOT).unwrap()
    }

    #[must_use]
    pub fn get_chestplate(&self) -> &ItemStack {
        self.get(Self::CHESTPLATE_SLOT).unwrap()
    }

    #[must_use]
    pub fn get_leggings(&self) -> &ItemStack {
        self.get(Self::LEGGINGS_SLOT).unwrap()
    }

    #[must_use]
    pub fn get_boots(&self) -> &ItemStack {
        self.get(Self::BOOTS_SLOT).unwrap()
    }

    pub fn try_add_item(&mut self, mut item: ItemStack) -> AddItemResult {
        let mut result = AddItemResult { remaining: None };

        // Try to add to hot bar (36-45) first, then the rest of the inventory (9-35)
        // try to stack first
        for slot in (36..=44).chain(9..36) {
            let Ok(add_slot) = self.try_add_to_slot(slot, &mut item, false) else {
                unreachable!("try_add_item should always return Ok");
            };

            match add_slot {
                TryAddSlot::Complete => {
                    return result;
                }
                TryAddSlot::Partial | TryAddSlot::Skipped => {}
            }
        }

        // Try to add to hot bar (36-44) first, then the rest of the inventory (9-35)
        // now try to add to empty slots
        for slot in (36..=44).chain(9..36) {
            let Ok(add_slot) = self.try_add_to_slot(slot, &mut item, true) else {
                unreachable!("try_add_item should always return Ok");
            };

            match add_slot {
                TryAddSlot::Complete => {
                    return result;
                }
                TryAddSlot::Partial | TryAddSlot::Skipped => {}
            }
        }

        // If there's any remaining item, set it in the result
        if item.count > 0 {
            result.remaining = Some(item);
        }

        result
    }
}

#[must_use]
pub fn slot_index_from_hand(hand_idx: u8) -> u16 {
    const HAND_START_SLOT: u16 = 36;
    const HAND_END_SLOT: u16 = 45;

    let hand_idx = u16::from(hand_idx);
    let hand_idx = hand_idx + HAND_START_SLOT;

    if hand_idx >= HAND_END_SLOT {
        return 0;
    }

    hand_idx
}

// todo: not sure if this is correct
pub const OFFHAND_SLOT: u16 = 45;

// #[cfg(test)]
// mod tests {
//     use valence_protocol::ItemKind;
//
//     use super::*;
//
//     #[test]
//     fn test_try_add_item_empty_inventory() {
//         let mut inventory = PlayerInventory::default();
//         let item = ItemStack::new(ItemKind::Stone, 64, None);
//         let result = inventory.try_add_item(item);
//
//         assert_eq!(result.changed_slots, vec![36]);
//         assert!(result.remaining.is_none());
//         assert_eq!(inventory.get(36).unwrap().count, 64);
//     }
//
//     #[test]
//     fn test_try_add_item_partially_filled_slot() {
//         let mut inventory = PlayerInventory::default();
//         inventory
//             .set(36, ItemStack::new(ItemKind::Stone, 32, None))
//             .unwrap();
//         let item = ItemStack::new(ItemKind::Stone, 64, None);
//         let result = inventory.try_add_item(item);
//
//         assert_eq!(result.changed_slots, vec![36, 37]);
//         assert!(result.remaining.is_none());
//         assert_eq!(inventory.get(36).unwrap().count, 64);
//         assert_eq!(inventory.get(37).unwrap().count, 32);
//     }
//
//     #[test]
//     fn test_try_add_item_full_inventory() {
//         let mut inventory = PlayerInventory::default();
//         for slot in 0..46 {
//             inventory
//                 .set(slot, ItemStack::new(ItemKind::Stone, 64, None))
//                 .unwrap();
//         }
//         let item = ItemStack::new(ItemKind::Stone, 64, None);
//         let result = inventory.try_add_item(item);
//
//         assert!(result.changed_slots.is_empty());
//         assert_eq!(
//             result.remaining,
//             Some(ItemStack::new(ItemKind::Stone, 64, None))
//         );
//     }
//
//     #[test]
//     fn test_try_add_item_different_items() {
//         let mut inventory = PlayerInventory::default();
//         inventory
//             .set(36, ItemStack::new(ItemKind::Stone, 64, None))
//             .unwrap();
//         let item = ItemStack::new(ItemKind::Dirt, 64, None);
//         let result = inventory.try_add_item(item);
//
//         assert_eq!(result.changed_slots, vec![37]);
//         assert!(result.remaining.is_none());
//         assert_eq!(inventory.get(36).unwrap().item, ItemKind::Stone);
//         assert_eq!(inventory.get(37).unwrap().item, ItemKind::Dirt);
//     }
//
//     #[test]
//     fn test_try_add_item_partial_stack() {
//         let mut inventory = PlayerInventory::default();
//         let item = ItemStack::new(ItemKind::Stone, 32, None);
//         let result = inventory.try_add_item(item);
//
//         assert_eq!(result.changed_slots, vec![36]);
//         assert!(result.remaining.is_none());
//         assert_eq!(inventory.get(36).unwrap().count, 32);
//     }
//
//     #[test]
//     fn test_try_add_item_multiple_partial_stacks() {
//         let mut inventory = PlayerInventory::default();
//         inventory
//             .set(36, ItemStack::new(ItemKind::Stone, 32, None))
//             .unwrap();
//         inventory
//             .set(37, ItemStack::new(ItemKind::Stone, 32, None))
//             .unwrap();
//         let item = ItemStack::new(ItemKind::Stone, 64, None);
//         let result = inventory.try_add_item(item);
//
//         assert!(result.remaining.is_none());
//         assert_eq!(inventory.get(36).unwrap().count, 64);
//         assert_eq!(inventory.get(37).unwrap().count, 64);
//         assert_eq!(inventory.get(38).unwrap().count, 0);
//
//         assert_eq!(result.changed_slots, vec![36, 37]);
//     }
//
//     #[test]
//     fn test_try_add_item_overflow() {
//         let mut inventory = PlayerInventory::default();
//         inventory
//             .set(36, ItemStack::new(ItemKind::Stone, 63, None))
//             .unwrap();
//         let item = ItemStack::new(ItemKind::Stone, 64, None);
//         let result = inventory.try_add_item(item);
//
//         assert_eq!(result.changed_slots, vec![36, 37]);
//         assert!(result.remaining.is_none());
//         assert_eq!(inventory.get(36).unwrap().count, 64);
//         assert_eq!(inventory.get(37).unwrap().count, 63);
//     }
// }

#[derive(Component)]
pub struct InventoryModule;

impl Module for InventoryModule {
    fn module(world: &World) {
        world.component::<PlayerInventory>();
    }
}
