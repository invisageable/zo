//! Roll-your-own archetype ECS for zo.
//!
//! Layer 2 of the rendering stack
//! (see `PLAN_ZO_ECS.md`). Single-threaded, owned by zo,
//! no third-party engine baggage. Same archetype layout
//! `bevy_ecs` / `hecs` use; we just don't ship the engine
//! around it.
//!
//! E1 surface: `World` + `Entity` + spawn/despawn + alive
//! check. Components and queries land in E2/E3.

mod archetype;
mod component;
mod entity;
mod query;
mod world;

pub use component::ComponentId;
pub use entity::Entity;
pub use query::{Query2, Query2Iter, Query2Mut};
pub use world::{EntityBuilder, World};
