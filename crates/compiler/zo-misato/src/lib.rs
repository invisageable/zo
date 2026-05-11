//! zo-misato — layer 3 of the rendering stack.
//!
//! Three.js-named runtime backing the user-facing
//! `compiler-lib/std/misato.zo`. The user writes scenes
//! with `Scene`, `PerspectiveCamera`, `Mesh`,
//! `BoxGeometry`, `MeshStandardMaterial`; this crate is
//! the Rust runtime those types delegate to via FFI.
//!
//! Single-threaded contract: every `__zo_misato_*` call
//! must run on the thread that called `init_window` (the
//! same thread that owns the raylib context). The runtime
//! lives in a `thread_local!` — accidental cross-thread
//! use sees `None` instead of silently corrupting state.
//!
//! M3 scope per `PLAN_ZO_MISATO.md`:
//!   - Each spawned cube is a real `zo-ecs` entity carrying
//!     `(Transform, RenderCube)` components. The render
//!     loop queries the World for `(RenderCube, Transform)`
//!     and dispatches `DrawCubeV` per row.
//!   - `Mesh` handles encode `(index, generation)` from
//!     [`zo_ecs::Entity::to_bits`], so handles survive
//!     despawn-recycle without aliasing.
//!   - Camera handles still index a flat `Vec<Camera3D>`
//!     (cameras are world-global, not per-entity).
//!   - `DrawCubeV` immediate primitive (no GPU mesh upload
//!     yet — M4 swaps to `DrawMesh` with a real
//!     `BoxGeometry` upload).

use std::cell::RefCell;

use zo_ecs::{ComponentId, Entity, World};

// --- Raylib FFI (3D mode) ------------------------------------

/// raylib's `Vector3 { float x, y, z; }` — 12 bytes, fits
/// in 3 S registers per AAPCS HFA rules.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vector3 {
  x: f32,
  y: f32,
  z: f32,
}

/// raylib's `Color { unsigned char r, g, b, a; }` — 4 bytes,
/// passed in the low half of a single GP register.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Color {
  r: u8,
  g: u8,
  b: u8,
  a: u8,
}

/// raylib's `Camera3D` — 44 bytes; AAPCS passes it via a
/// hidden caller-allocated copy and a pointer in X0.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Camera3D {
  position: Vector3,
  target: Vector3,
  up: Vector3,
  fovy: f32,
  /// 0 = perspective, 1 = orthographic.
  projection: i32,
}

unsafe extern "C" {
  fn BeginMode3D(camera: Camera3D);
  fn EndMode3D();
  fn DrawCubeV(position: Vector3, size: Vector3, color: Color);
}

// --- ECS components ------------------------------------------

/// Per-entity world-space position. 12 bytes, naturally
/// aligned to 4 (matches `Vector3`'s alignment).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Transform {
  position: Vector3,
}

/// Per-entity render data — what to draw at the entity's
/// `Transform.position`. Mesh + material folded into one
/// component so the render loop can use `Query2`
/// (M3-friendly; Query3 lands when an unrelated consumer
/// needs it). 16 bytes (12 size + 4 color).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct RenderCube {
  size: Vector3,
  color: Color,
}

// --- Runtime state -------------------------------------------

/// Owned by the thread-local runtime slot. Holds the ECS
/// World plus a few flat tables for resources the user
/// addresses by handle (geometries, materials, cameras).
struct RuntimeState {
  world: World,
  /// Component id for `Transform`, registered once at init.
  transform_id: ComponentId,
  /// Component id for `RenderCube`, registered once at init.
  render_cube_id: ComponentId,
  /// Geometry table — handle = 1-indexed `Vec` index. M1+M2
  /// referenced these by handle from `spawn_box_mesh`; M3
  /// keeps the table because cubes still need a size source.
  geoms: Vec<Vector3>,
  /// Material table — same scheme as `geoms`.
  mats: Vec<Color>,
  /// Camera table. Handles are 1-indexed; handle 0 means
  /// "use the default camera at (3,3,3) → origin".
  cameras: Vec<Camera3D>,
}

impl RuntimeState {
  fn new() -> Self {
    let mut world = World::new();
    let transform_id = world.register::<Transform>();
    let render_cube_id = world.register::<RenderCube>();
    Self {
      world,
      transform_id,
      render_cube_id,
      geoms: Vec::new(),
      mats: Vec::new(),
      cameras: Vec::new(),
    }
  }
}

/// Camera used when the user's render call passes handle
/// 0 (or an out-of-range handle). Mirrors M1 behaviour.
const DEFAULT_CAMERA: Camera3D = Camera3D {
  position: Vector3 { x: 3.0, y: 3.0, z: 3.0 },
  target: Vector3 { x: 0.0, y: 0.0, z: 0.0 },
  up: Vector3 { x: 0.0, y: 1.0, z: 0.0 },
  fovy: 45.0,
  projection: 0,
};

thread_local! {
  /// Per-thread runtime slot. `init_world` populates it;
  /// `destroy_world` clears it. All other FFI calls borrow
  /// it mutably for one operation.
  static RUNTIME: RefCell<Option<RuntimeState>> = const { RefCell::new(None) };
}

/// Pack `(r, g, b, a)` u8s out of zo's 0xRRGGBBAA u32. zo
/// passes packed colors as `int` (sign-extended to 64-bit
/// in X0); we read the low 32 bits as the RGBA literal.
fn unpack_color(rgba: u32) -> Color {
  Color {
    r: ((rgba >> 24) & 0xFF) as u8,
    g: ((rgba >> 16) & 0xFF) as u8,
    b: ((rgba >> 8) & 0xFF) as u8,
    a: (rgba & 0xFF) as u8,
  }
}

// --- FFI surface (per PLAN_ZO_MISATO.md) ---------------------

/// Initialise the runtime. Returns the world handle
/// (always `1` — single-world model).
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_init_world() -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    if slot.is_none() {
      *slot = Some(RuntimeState::new());
    }
  });
  1
}

/// Tear down the runtime. Idempotent.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_destroy_world(_handle: i64) {
  RUNTIME.with(|cell| {
    *cell.borrow_mut() = None;
  });
}

/// Allocate a `BoxGeometry` of the given dimensions.
/// Returns 1-indexed handle into the runtime's geom table.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_make_box_geom(w: f32, h: f32, d: f32) -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };
    state.geoms.push(Vector3 { x: w, y: h, z: d });
    state.geoms.len() as i64
  })
}

/// Allocate a `MeshStandardMaterial` from a packed RGBA
/// color (`0xRRGGBBAA`). Returns 1-indexed handle.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_make_standard_mat(color_rgba: u32) -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };
    state.mats.push(unpack_color(color_rgba));
    state.mats.len() as i64
  })
}

/// Spawn a cube entity at `(x, y, z)` from the given
/// geometry + material handles. Returns the entity handle
/// — `(generation << 32) | index` packed via
/// [`Entity::to_bits`].
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_spawn_box_mesh(
  geom: i64,
  mat: i64,
  x: f32,
  y: f32,
  z: f32,
) -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };

    let geom_idx = (geom as usize).saturating_sub(1);
    let mat_idx = (mat as usize).saturating_sub(1);
    let size = state.geoms.get(geom_idx).copied().unwrap_or(Vector3 {
      x: 1.0,
      y: 1.0,
      z: 1.0,
    });
    let color = state.mats.get(mat_idx).copied().unwrap_or(Color {
      r: 255,
      g: 255,
      b: 255,
      a: 255,
    });

    let transform_id = state.transform_id;
    let render_cube_id = state.render_cube_id;

    let entity = state
      .world
      .spawn()
      .with(transform_id, Transform { position: Vector3 { x, y, z } })
      .with(render_cube_id, RenderCube { size, color })
      .build();

    entity.to_bits() as i64
  })
}

/// Update the position of a previously-spawned mesh. No-op
/// if the handle is dead or doesn't carry a `Transform`.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_mesh_set_position(
  mesh: i64,
  x: f32,
  y: f32,
  z: f32,
) {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return;
    };
    let entity = Entity::from_bits(mesh as u64);
    state.world.set(entity, Transform { position: Vector3 { x, y, z } });
  });
}

/// Allocate a perspective camera with the given intrinsics.
/// Default position `(3, 3, 3)`, target `(0, 0, 0)`, up
/// `(0, 1, 0)` — overridable via `set_position` / `look_at`.
/// Returns 1-indexed handle.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_camera_new(
  fov: f32,
  _aspect: f32,
  _near: f32,
  _far: f32,
) -> i64 {
  // raylib's Camera3D doesn't carry aspect/near/far — those
  // are derived from the framebuffer + projection. We accept
  // them in the public API for forward compat (and to match
  // the Three.js shape) but only fovy lands on Camera3D.
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };
    let mut cam = DEFAULT_CAMERA;
    cam.fovy = fov;
    state.cameras.push(cam);
    state.cameras.len() as i64
  })
}

/// Move a camera to `(x, y, z)`.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_camera_set_position(
  cam: i64,
  x: f32,
  y: f32,
  z: f32,
) {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return;
    };
    let idx = (cam as usize).saturating_sub(1);
    if let Some(c) = state.cameras.get_mut(idx) {
      c.position = Vector3 { x, y, z };
    }
  });
}

/// Aim a camera at `(x, y, z)` (world space).
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_camera_look_at(
  cam: i64,
  x: f32,
  y: f32,
  z: f32,
) {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return;
    };
    let idx = (cam as usize).saturating_sub(1);
    if let Some(c) = state.cameras.get_mut(idx) {
      c.target = Vector3 { x, y, z };
    }
  });
}

/// `scene_add` is a no-op in M3+ — every spawned mesh is
/// already a renderable ECS entity, so there's nothing for
/// the scene to track separately. Kept as part of the FFI
/// surface so M1/M2 user code continues to compile.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_scene_add(_scene: i64, _ent: i64) {}

/// Drive one render pass. Looks up the camera by handle
/// (`cam_handle == 0` falls back to the runtime default at
/// `(3,3,3) → origin`); queries the World for every
/// `(RenderCube, Transform)` row and dispatches `DrawCubeV`
/// per row. The user's render loop must wrap this in
/// `BeginDrawing` / `EndDrawing` — we only manage the 3D
/// mode bracketing.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_scene_render(_scene: i64, cam_handle: i64) {
  RUNTIME.with(|cell| {
    let slot = cell.borrow();
    let Some(state) = slot.as_ref() else {
      return;
    };

    let camera = if cam_handle == 0 {
      DEFAULT_CAMERA
    } else {
      state
        .cameras
        .get((cam_handle as usize).saturating_sub(1))
        .copied()
        .unwrap_or(DEFAULT_CAMERA)
    };

    unsafe {
      BeginMode3D(camera);
      for (rc, t) in state.world.query2::<RenderCube, Transform>().iter() {
        DrawCubeV(t.position, rc.size, rc.color);
      }
      EndMode3D();
    }
  });
}
