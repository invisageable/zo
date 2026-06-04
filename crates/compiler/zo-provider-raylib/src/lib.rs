//! raylib can't be bound from zo directly: it passes
//! `Texture2D` / `Image` by value, and zo can't build
//! C-packed structs. This shim builds them and forwards.

use std::os::raw::{c_char, c_int, c_void};

// ===== raylib C structs (native, packed) =====

/// raylib `Vector2` — two `f32`. Passed by value as an HFA.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Vector2 {
  x: f32,
  y: f32,
}

/// raylib `Color` — four 8-bit channels.
#[repr(C)]
#[derive(Clone, Copy)]
struct Color {
  r: u8,
  g: u8,
  b: u8,
  a: u8,
}

/// raylib `Image` — CPU pixel buffer + geometry.
#[repr(C)]
struct Image {
  data: *mut c_void,
  width: c_int,
  height: c_int,
  mipmaps: c_int,
  format: c_int,
}

/// raylib `Texture2D` — GPU texture handle + geometry.
#[repr(C)]
#[derive(Clone, Copy)]
struct Texture2D {
  id: u32,
  width: c_int,
  height: c_int,
  mipmaps: c_int,
  format: c_int,
}

/// `PixelFormat::PIXELFORMAT_UNCOMPRESSED_R8G8B8A8`.
const PIXELFORMAT_UNCOMPRESSED_R8G8B8A8: c_int = 7;

const WHITE: Color = Color {
  r: 255,
  g: 255,
  b: 255,
  a: 255,
};

/// Decode a zo packed-`int` color into raylib's `Color`.
/// Byte 0 → `r`, …, byte 3 → `a` — the same little-endian
/// mapping the old direct-`Color`-in-a-register binding used,
/// so existing programs render identically.
#[inline]
fn color(c: i64) -> Color {
  Color {
    r: (c & 0xFF) as u8,
    g: ((c >> 8) & 0xFF) as u8,
    b: ((c >> 16) & 0xFF) as u8,
    a: ((c >> 24) & 0xFF) as u8,
  }
}

// ===== system raylib (resolved by build.rs) =====

unsafe extern "C" {
  fn InitWindow(width: c_int, height: c_int, title: *const c_char);
  fn CloseWindow();
  fn WindowShouldClose() -> bool;
  fn ClearBackground(color: Color);
  fn BeginDrawing();
  fn EndDrawing();
  fn SetTargetFPS(fps: c_int);
  fn GetFrameTime() -> f32;
  fn GetFPS() -> c_int;
  fn IsKeyPressed(key: c_int) -> bool;
  fn IsKeyDown(key: c_int) -> bool;
  fn IsKeyUp(key: c_int) -> bool;
  fn GetMousePosition() -> Vector2;
  fn DrawCircle(center_x: c_int, center_y: c_int, radius: f32, color: Color);
  fn DrawCircleV(center: Vector2, radius: f32, color: Color);
  fn DrawRectangle(
    pos_x: c_int,
    pos_y: c_int,
    width: c_int,
    height: c_int,
    color: Color,
  );
  fn DrawRectangleLines(
    pos_x: c_int,
    pos_y: c_int,
    width: c_int,
    height: c_int,
    color: Color,
  );
  fn DrawPixel(pos_x: c_int, pos_y: c_int, color: Color);
  fn DrawText(
    text: *const c_char,
    pos_x: c_int,
    pos_y: c_int,
    font_size: c_int,
    color: Color,
  );
  fn DrawFPS(pos_x: c_int, pos_y: c_int);
  fn LoadTextureFromImage(image: Image) -> Texture2D;
  fn UpdateTexture(texture: Texture2D, pixels: *const c_void);
  fn DrawTexture(texture: Texture2D, pos_x: c_int, pos_y: c_int, tint: Color);
  fn UnloadTexture(texture: Texture2D);
}

// ===== Window =====

/// # Safety
///
/// `title` must be a live NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_raylib_init_window(
  width: i64,
  height: i64,
  title: *const c_char,
) {
  unsafe { InitWindow(width as c_int, height as c_int, title) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_close_window() {
  unsafe { CloseWindow() };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_window_should_close() -> bool {
  unsafe { WindowShouldClose() }
}

// ===== Drawing =====

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_clear_background(color_packed: i64) {
  unsafe { ClearBackground(color(color_packed)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_begin_drawing() {
  unsafe { BeginDrawing() };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_end_drawing() {
  unsafe { EndDrawing() };
}

// ===== Timing =====

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_set_target_fps(fps: i64) {
  unsafe { SetTargetFPS(fps as c_int) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_get_frame_time() -> f32 {
  unsafe { GetFrameTime() }
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_get_fps() -> i64 {
  unsafe { GetFPS() as i64 }
}

// ===== Input =====

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_is_key_pressed(key: i64) -> bool {
  unsafe { IsKeyPressed(key as c_int) }
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_is_key_down(key: i64) -> bool {
  unsafe { IsKeyDown(key as c_int) }
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_is_key_up(key: i64) -> bool {
  unsafe { IsKeyUp(key as c_int) }
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_get_mouse_position() -> Vector2 {
  unsafe { GetMousePosition() }
}

// ===== Shapes =====

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_draw_circle(
  x: i64,
  y: i64,
  radius: f32,
  color_packed: i64,
) {
  unsafe { DrawCircle(x as c_int, y as c_int, radius, color(color_packed)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_draw_circle_v(
  center: Vector2,
  radius: f32,
  color_packed: i64,
) {
  unsafe { DrawCircleV(center, radius, color(color_packed)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_draw_rectangle(
  x: i64,
  y: i64,
  width: i64,
  height: i64,
  color_packed: i64,
) {
  unsafe {
    DrawRectangle(
      x as c_int,
      y as c_int,
      width as c_int,
      height as c_int,
      color(color_packed),
    )
  };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_draw_rectangle_lines(
  x: i64,
  y: i64,
  width: i64,
  height: i64,
  color_packed: i64,
) {
  unsafe {
    DrawRectangleLines(
      x as c_int,
      y as c_int,
      width as c_int,
      height as c_int,
      color(color_packed),
    )
  };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_draw_pixel(x: i64, y: i64, color_packed: i64) {
  unsafe { DrawPixel(x as c_int, y as c_int, color(color_packed)) };
}

// ===== Text =====

/// # Safety
///
/// `text` must be a live NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_raylib_draw_text(
  text: *const c_char,
  x: i64,
  y: i64,
  font_size: i64,
  color_packed: i64,
) {
  unsafe {
    DrawText(
      text,
      x as c_int,
      y as c_int,
      font_size as c_int,
      color(color_packed),
    )
  };
}

#[unsafe(no_mangle)]
pub extern "C" fn zo_raylib_draw_fps(x: i64, y: i64) {
  unsafe { DrawFPS(x as c_int, y as c_int) };
}

// ===== Textures =====
//
// `Texture2D` / `Image` are packed C structs zo can't pass by
// value, so the shim builds them and hands zo an opaque `i64`
// handle (a boxed `Texture2D`). `0` is never a valid handle.

/// Upload a `width * height` RGBA buffer (R,G,B,A) to the GPU
/// and return a texture handle.
///
/// # Safety
///
/// `pixels` must be a live buffer of `width * height * 4`
/// bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_raylib_load_texture(
  pixels: *const c_void,
  width: i64,
  height: i64,
) -> i64 {
  let image = Image {
    data: pixels as *mut c_void,
    width: width as c_int,
    height: height as c_int,
    mipmaps: 1,
    format: PIXELFORMAT_UNCOMPRESSED_R8G8B8A8,
  };

  // SAFETY: `image` mirrors raylib's layout; `pixels` is the
  // caller's live buffer, which raylib copies to the GPU.
  let texture = unsafe { LoadTextureFromImage(image) };

  Box::into_raw(Box::new(texture)) as i64
}

/// Re-upload fresh pixels into an existing texture.
///
/// # Safety
///
/// `handle` must come from `zo_raylib_load_texture` and not be
/// unloaded; `pixels` must match the texture's dimensions.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_raylib_update_texture(
  handle: i64,
  pixels: *const c_void,
) {
  if handle == 0 {
    return;
  }

  // SAFETY: caller contract — a live boxed `Texture2D`.
  let texture = unsafe { &*(handle as *const Texture2D) };

  // SAFETY: same; `pixels` is the caller's live buffer.
  unsafe { UpdateTexture(*texture, pixels) };
}

/// Draw the texture with its top-left at `(x, y)`.
///
/// # Safety
///
/// `handle` must come from `zo_raylib_load_texture` and not be
/// unloaded.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_raylib_draw_texture(handle: i64, x: i64, y: i64) {
  if handle == 0 {
    return;
  }

  // SAFETY: caller contract — a live boxed `Texture2D`.
  let texture = unsafe { &*(handle as *const Texture2D) };

  unsafe { DrawTexture(*texture, x as c_int, y as c_int, WHITE) };
}

/// Free the GPU texture and reclaim the handle's box.
///
/// # Safety
///
/// `handle` must come from `zo_raylib_load_texture` and be
/// unloaded exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zo_raylib_unload_texture(handle: i64) {
  if handle == 0 {
    return;
  }

  // SAFETY: caller contract — reclaim ownership, release GPU.
  let texture = unsafe { Box::from_raw(handle as *mut Texture2D) };

  unsafe { UnloadTexture(*texture) };
}
