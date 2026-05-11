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
//! M2 scope per `PLAN_ZO_MISATO.md`:
//!   - Scene-local cube list (no ECS yet — M3 promotes it)
//!   - Real camera handles: `PerspectiveCamera::new` builds
//!     a runtime `Camera3D`; `set_position` / `look_at`
//!     mutate it; render dereferences the handle each frame
//!   - Per-mesh repositioning via `Mesh::set_position`
//!   - `DrawCubeV` immediate primitive (no GPU mesh upload
//!     yet — M4 swaps to `DrawMesh` with a real
//!     `BoxGeometry` upload)

use std::cell::RefCell;

use zo_ecs::World;

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

// --- Runtime state -------------------------------------------

/// One spawned cube. M1 stores everything inline; M3
/// promotes these into ECS entities with `(Mesh,
/// Transform, Material)` components.
#[derive(Clone, Copy, Debug)]
struct CubeData {
  size: Vector3,
  position: Vector3,
  color: Color,
}

/// Pending allocations indexed by the handles handed back
/// to zo. Handles are 1-indexed so 0 stays a sentinel
/// "null handle" value.
struct RuntimeState {
  // Reserved for M3+ — the World will own the (Mesh,
  // Transform, Material) entities once the render system
  // queries it. M1/M2 keeps a flat Vec, so the World is
  // built but unused; this matches the plan's M1+M2
  // shortcut.
  #[allow(dead_code)]
  world: World,
  geoms: Vec<Vector3>,
  mats: Vec<Color>,
  cubes: Vec<CubeData>,
  /// Camera table. Handles are 1-indexed; handle 0 means
  /// "use the default camera at (3,3,3) → origin".
  cameras: Vec<Camera3D>,
  /// Indices into `cubes` that the scene draws each frame.
  /// One scene per world for M1/M2 — the scene handle is
  /// always `1` and is ignored.
  scene: Vec<usize>,
}

impl RuntimeState {
  fn new() -> Self {
    Self {
      world: World::new(),
      geoms: Vec::new(),
      mats: Vec::new(),
      cubes: Vec::new(),
      cameras: Vec::new(),
      scene: Vec::new(),
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
/// (always `1` — single-world model in M1).
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
/// Returns 1-indexed handle.
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

/// Spawn a `Mesh` entity at `(x, y, z)` from the given
/// geometry + material handles. Returns 1-indexed handle.
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

    state.cubes.push(CubeData {
      size,
      position: Vector3 { x, y, z },
      color,
    });
    state.cubes.len() as i64
  })
}

/// Update the position of a previously-spawned mesh.
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
    let idx = (mesh as usize).saturating_sub(1);
    if let Some(cube) = state.cubes.get_mut(idx) {
      cube.position = Vector3 { x, y, z };
    }
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

/// Append entity handle to the scene's draw list.
#[unsafe(no_mangle)]
pub extern "C" fn __zo_misato_scene_add(_scene: i64, ent: i64) {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return;
    };
    let ent_idx = (ent as usize).saturating_sub(1);
    if ent_idx < state.cubes.len() {
      state.scene.push(ent_idx);
    }
  });
}

/// Drive one render pass. Looks up the camera by handle
/// (`cam_handle == 0` falls back to the runtime default at
/// `(3,3,3) → origin`); iterates the scene list and
/// dispatches `DrawCubeV` per cube. The user's render
/// loop must wrap this in `BeginDrawing` / `EndDrawing` —
/// we only manage the 3D mode bracketing.
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
      for &ent_idx in &state.scene {
        let cube = state.cubes[ent_idx];
        DrawCubeV(cube.position, cube.size, cube.color);
      }
      EndMode3D();
    }
  });
}
