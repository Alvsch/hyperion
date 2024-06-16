//! Flecs components which are used for events.

use flecs_ecs::{core::Entity, macros::Component};
use glam::Vec3;

/// Represents an attack action by an entity in the game.
#[derive(Component, Copy, Clone, Debug, PartialEq)]
pub struct AttackEntity {
    /// The location of the player that is attacking.
    pub from_pos: Vec3,
    /// The entity that is performing the attack.
    pub from: Entity,
    /// The damage dealt by the attack. This corresponds to the same unit as [`crate::component::Health`].
    pub damage: f32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[repr(i32)]
#[allow(missing_docs, reason = "self explanatory")]
pub enum Posture {
    Standing = 0,
    FallFlying = 1,
    Sleeping = 2,
    Swimming = 3,
    SpinAttack = 4,
    Sneaking = 5,
    LongJumping = 6,
    Dying = 7,
    Croaking = 8,
    UsingTongue = 9,
    Sitting = 10,
    Roaring = 11,
    Sniffing = 12,
    Emerging = 13,
    Digging = 14,
}

/// <https://wiki.vg/index.php?title=Protocol&oldid=18375#Set_Entity_Metadata>
#[derive(Component, Copy, Clone, Debug, PartialEq, Eq)]
pub struct PostureUpdate {
    /// The new posture of the entity.
    pub state: Posture,
}
