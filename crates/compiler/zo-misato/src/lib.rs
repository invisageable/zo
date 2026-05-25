//! zo-misato — runtime backing the user-facing
//! `compiler-lib/std/misato.zo`. The user writes scenes
//! with `Scene`, `PerspectiveCamera`, `Mesh`,
//! `BoxGeometry`, `MeshStandardMaterial`; this crate is
//! the Rust runtime those types delegate to via FFI.
//!
//! Single-threaded contract: every `zo_misato_*` call
//! must run on the thread that called `init_window` (the
//! same thread that owns the raylib context). The runtime
//! lives in a `thread_local!` — accidental cross-thread
//! use sees `None` instead of silently corrupting state.

use std::cell::RefCell;

use zo_ecs::{ComponentId, Entity, World};

// --- Raylib FFI types ----------------------------------------

/// raylib `Vector3`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Vector3 {
  x: f32,
  y: f32,
  z: f32,
}

/// raylib `Color` (RGBA bytes).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Color {
  r: u8,
  g: u8,
  b: u8,
  a: u8,
}

/// raylib `Camera3D`. 44 bytes — AAPCS passes by hidden
/// caller-allocated copy; the pointer goes in X8.
/// <https://www.raylib.com/cheatsheet/cheatsheet.html#types>
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

/// raylib `Matrix`, column-major 4×4. We construct it
/// inline (`matrix_translate`) instead of calling raylib's
/// `MatrixTranslate` — saves one FFI hop per cube per
/// frame.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Matrix {
  m0: f32,
  m4: f32,
  m8: f32,
  m12: f32,
  m1: f32,
  m5: f32,
  m9: f32,
  m13: f32,
  m2: f32,
  m6: f32,
  m10: f32,
  m14: f32,
  m3: f32,
  m7: f32,
  m11: f32,
  m15: f32,
}

/// raylib `Texture2D`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Texture2D {
  id: u32,
  width: i32,
  height: i32,
  mipmaps: i32,
  format: i32,
}

/// raylib `MaterialMap`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct MaterialMap {
  texture: Texture2D,
  color: Color,
  value: f32,
}

/// raylib `Shader` — opaque handle pair (program id +
/// uniform location array). Replaced wholesale by
/// `LoadShaderFromMemory` for `MeshNormalMaterial`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Shader {
  id: u32,
  locs: *mut i32,
}

/// raylib `Material`. We override `(*maps)[0].color` after
/// `LoadMaterialDefault` returns — that's the documented
/// recolor path (slot 0 is `MATERIAL_MAP_DIFFUSE`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Material {
  shader: Shader,
  maps: *mut MaterialMap,
  params: [f32; 4],
}

/// raylib `Mesh`. ~120 bytes — AAPCS passes by hidden
/// caller-allocated copy. We treat it as opaque: the inner
/// pointers belong to raylib (allocated by `GenMesh*`,
/// freed by `UnloadMesh`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct RaylibMesh {
  vertex_count: i32,
  triangle_count: i32,
  vertices: *mut f32,
  texcoords: *mut f32,
  texcoords2: *mut f32,
  normals: *mut f32,
  tangents: *mut f32,
  colors: *mut u8,
  indices: *mut u16,
  anim_vertices: *mut f32,
  anim_normals: *mut f32,
  bone_ids: *mut u8,
  bone_weights: *mut f32,
  bone_matrices: *mut Matrix,
  bone_count: i32,
  vao_id: u32,
  vbo_id: *mut u32,
}

unsafe extern "C" {
  fn BeginMode3D(camera: Camera3D);
  fn EndMode3D();

  fn GenMeshCube(width: f32, height: f32, length: f32) -> RaylibMesh;
  fn GenMeshSphere(radius: f32, rings: i32, slices: i32) -> RaylibMesh;
  fn GenMeshPlane(
    width: f32,
    length: f32,
    res_x: i32,
    res_z: i32,
  ) -> RaylibMesh;
  fn UnloadMesh(mesh: RaylibMesh);

  fn LoadMaterialDefault() -> Material;
  fn UnloadMaterial(material: Material);

  fn LoadShaderFromMemory(vs_code: *const u8, fs_code: *const u8) -> Shader;

  fn DrawMesh(mesh: RaylibMesh, material: Material, transform: Matrix);
}

/// Vertex shader for `MeshNormalMaterial`. Forwards the
/// world-space normal to the fragment stage. Uses
/// raylib's default attribute names + uniform names so
/// no extra location lookups are needed.
const NORMAL_MATERIAL_VS: &[u8] = b"\
#version 330\n\
in vec3 vertexPosition;\n\
in vec3 vertexNormal;\n\
uniform mat4 mvp;\n\
uniform mat4 matModel;\n\
out vec3 fragNormal;\n\
void main() {\n\
  fragNormal = mat3(matModel) * vertexNormal;\n\
  gl_Position = mvp * vec4(vertexPosition, 1.0);\n\
}\n\0";

/// Fragment shader for `MeshNormalMaterial` — outputs
/// `color = normalize(normal) * 0.5 + 0.5`.
const NORMAL_MATERIAL_FS: &[u8] = b"\
#version 330\n\
in vec3 fragNormal;\n\
out vec4 finalColor;\n\
void main() {\n\
  finalColor = vec4(normalize(fragNormal) * 0.5 + 0.5, 1.0);\n\
}\n\0";

// --- ECS components ------------------------------------------

/// Per-entity model matrix, cached at spawn /
/// `set_position`. The render loop reads it directly, so
/// 1000 cubes × 60 FPS doesn't pay for 60k matrix rebuilds
/// per second.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Transform {
  model: Matrix,
}

/// Indirection from entity → uploaded `RaylibMesh` +
/// `Material`. Sharing the same `(geom_idx, mat_idx)`
/// across entities reuses one GPU upload.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct MeshRef {
  geom_idx: u32,
  mat_idx: u32,
}

// --- Runtime state -------------------------------------------

/// All public handles (geom / mat / camera / mesh) are
/// 1-indexed into these tables; `0` is the null sentinel.
/// `geoms` and `mats` own GPU resources — see
/// `impl Drop` for cleanup.
struct RuntimeState {
  world: World,
  transform_id: ComponentId,
  mesh_ref_id: ComponentId,
  geoms: Vec<RaylibMesh>,
  mats: Vec<Material>,
  cameras: Vec<Camera3D>,
}

impl RuntimeState {
  fn new() -> Self {
    let mut world = World::new();
    let transform_id = world.register::<Transform>();
    let mesh_ref_id = world.register::<MeshRef>();
    Self {
      world,
      transform_id,
      mesh_ref_id,
      geoms: Vec::new(),
      mats: Vec::new(),
      cameras: Vec::new(),
    }
  }
}

impl Drop for RuntimeState {
  fn drop(&mut self) {
    // VRAM can outlive the process on some drivers — must
    // explicitly hand each upload back to raylib before the
    // runtime slot disappears.
    for mesh in self.geoms.drain(..) {
      unsafe { UnloadMesh(mesh) };
    }
    for material in self.mats.drain(..) {
      unsafe { UnloadMaterial(material) };
    }
  }
}

/// Fallback for `cam_handle == 0` and any out-of-range
/// camera lookup.
const DEFAULT_CAMERA: Camera3D = Camera3D {
  position: Vector3 {
    x: 3.0,
    y: 3.0,
    z: 3.0,
  },
  target: Vector3 {
    x: 0.0,
    y: 0.0,
    z: 0.0,
  },
  up: Vector3 {
    x: 0.0,
    y: 1.0,
    z: 0.0,
  },
  fovy: 45.0,
  projection: 0,
};

thread_local! {
  // Per-thread isolation enforces the single-threaded
  // raylib contract: a stray cross-thread call sees `None`
  // and no-ops instead of corrupting the GL context.
  static RUNTIME: RefCell<Option<RuntimeState>> = const { RefCell::new(None) };
}

fn unpack_color(rgba: u32) -> Color {
  Color {
    r: ((rgba >> 24) & 0xFF) as u8,
    g: ((rgba >> 16) & 0xFF) as u8,
    b: ((rgba >> 8) & 0xFF) as u8,
    a: (rgba & 0xFF) as u8,
  }
}

/// Public handles are 1-indexed; `saturating_sub` keeps
/// the null sentinel (`0`) from underflowing into a wild
/// table read. Callers still bounds-check via `Vec::get`.
#[inline]
fn handle_to_idx(handle: i64) -> usize {
  (handle as usize).saturating_sub(1)
}

#[inline]
fn matrix_translate(x: f32, y: f32, z: f32) -> Matrix {
  Matrix {
    m0: 1.0,
    m4: 0.0,
    m8: 0.0,
    m12: x,
    m1: 0.0,
    m5: 1.0,
    m9: 0.0,
    m13: y,
    m2: 0.0,
    m6: 0.0,
    m10: 1.0,
    m14: z,
    m3: 0.0,
    m7: 0.0,
    m11: 0.0,
    m15: 1.0,
  }
}

// --- FFI surface --------------------------------------------

/// Idempotent. Returns the world handle (always `1` —
/// single-world model).
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_init_world() -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    if slot.is_none() {
      *slot = Some(RuntimeState::new());
    }
  });
  1
}

/// Idempotent. Triggers `RuntimeState::Drop`, which
/// reclaims every uploaded mesh + material before the
/// next `init_world`.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_destroy_world(_handle: i64) {
  RUNTIME.with(|cell| {
    *cell.borrow_mut() = None;
  });
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_make_box_geom(w: f32, h: f32, d: f32) -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };

    // raylib 5.5's `GenMesh*` family already calls
    // `UploadMesh` internally — calling it again triggers
    // "re-load" warnings and silently keeps the first
    // upload.
    let mesh = unsafe { GenMeshCube(w, h, d) };

    state.geoms.push(mesh);
    state.geoms.len() as i64
  })
}

/// `rings` = horizontal slices, `slices` = vertical
/// slices (raylib's `GenMeshSphere` parameter order).
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_make_sphere_geom(
  radius: f32,
  rings: i64,
  slices: i64,
) -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };

    let mesh = unsafe { GenMeshSphere(radius, rings as i32, slices as i32) };
    state.geoms.push(mesh);
    state.geoms.len() as i64
  })
}

/// raylib's `GenMeshPlane` builds an XZ plane at Y=0
/// with normals pointing `+Y`.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_make_plane_geom(
  width: f32,
  length: f32,
  res_x: i64,
  res_z: i64,
) -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };

    let mesh =
      unsafe { GenMeshPlane(width, length, res_x as i32, res_z as i32) };
    state.geoms.push(mesh);
    state.geoms.len() as i64
  })
}

/// Color is packed `0xRRGGBBAA`.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_make_standard_mat(color_rgba: u32) -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };

    let material = unsafe { LoadMaterialDefault() };
    // raylib's default material allocates a `MaterialMap`
    // array of length `MAX_MATERIAL_MAPS`; slot 0 is
    // `MATERIAL_MAP_DIFFUSE`. Overriding its color is the
    // documented recolor path.
    if !material.maps.is_null() {
      unsafe {
        (*material.maps).color = unpack_color(color_rgba);
      }
    }

    state.mats.push(material);
    state.mats.len() as i64
  })
}

/// `MeshNormalMaterial`: each fragment colored
/// `normalize(normal) * 0.5 + 0.5`. Each call allocates a
/// fresh shader; `UnloadMaterial` releases it on cleanup.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_make_normal_mat() -> i64 {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return 0;
    };

    let mut material = unsafe { LoadMaterialDefault() };
    let shader = unsafe {
      LoadShaderFromMemory(
        NORMAL_MATERIAL_VS.as_ptr(),
        NORMAL_MATERIAL_FS.as_ptr(),
      )
    };
    material.shader = shader;

    state.mats.push(material);
    state.mats.len() as i64
  })
}

/// Spawn a cube entity at `(x, y, z)` referencing the
/// given geometry + material handles. Returns the entity
/// packed via [`Entity::to_bits`].
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_spawn_box_mesh(
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

    let geom_idx = (geom as u32).saturating_sub(1);
    let mat_idx = (mat as u32).saturating_sub(1);

    let transform_id = state.transform_id;
    let mesh_ref_id = state.mesh_ref_id;

    let entity = state
      .world
      .spawn()
      .with(
        transform_id,
        Transform {
          model: matrix_translate(x, y, z),
        },
      )
      .with(mesh_ref_id, MeshRef { geom_idx, mat_idx })
      .build();

    entity.to_bits() as i64
  })
}

/// No-op when `mesh` is dead or lacks a `Transform`.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_mesh_set_position(
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
    state.world.set(
      entity,
      Transform {
        model: matrix_translate(x, y, z),
      },
    );
  });
}

/// Default camera placed at `DEFAULT_CAMERA` — override
/// via `set_position` / `look_at`. `aspect`/`near`/`far`
/// are reserved for the public API but unused here:
/// raylib derives them from the framebuffer + projection,
/// so `Camera3D` only carries `fovy`.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_camera_new(
  fov: f32,
  _aspect: f32,
  _near: f32,
  _far: f32,
) -> i64 {
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

#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_camera_set_position(
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
    let idx = handle_to_idx(cam);
    if let Some(c) = state.cameras.get_mut(idx) {
      c.position = Vector3 { x, y, z };
    }
  });
}

/// Camera target in world space.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_camera_look_at(cam: i64, x: f32, y: f32, z: f32) {
  RUNTIME.with(|cell| {
    let mut slot = cell.borrow_mut();
    let Some(state) = slot.as_mut() else {
      return;
    };
    let idx = handle_to_idx(cam);
    if let Some(c) = state.cameras.get_mut(idx) {
      c.target = Vector3 { x, y, z };
    }
  });
}

/// No-op — every spawned mesh is already a renderable ECS
/// entity. Kept on the FFI surface so the user-facing
/// `scene.add(mesh)` call shape survives.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_scene_add(_scene: i64, _ent: i64) {}

/// Caller wraps this in `BeginDrawing` / `EndDrawing`;
/// we only manage the 3D-mode bracketing.
#[unsafe(no_mangle)]
pub extern "C" fn zo_misato_scene_render(_scene: i64, cam_handle: i64) {
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
        .get(handle_to_idx(cam_handle))
        .copied()
        .unwrap_or(DEFAULT_CAMERA)
    };

    let geom_count = state.geoms.len();
    let mat_count = state.mats.len();

    unsafe {
      BeginMode3D(camera);
      for (mr, t) in state.world.query2::<MeshRef, Transform>().iter() {
        let geom_idx = mr.geom_idx as usize;
        let mat_idx = mr.mat_idx as usize;
        // Out-of-range handle = programmer error, not
        // crash-worthy. Skip the row.
        if geom_idx >= geom_count || mat_idx >= mat_count {
          continue;
        }
        DrawMesh(state.geoms[geom_idx], state.mats[mat_idx], t.model);
      }
      EndMode3D();
    }
  });
}
