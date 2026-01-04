//! Example demonstrating the #[derive(Tweenable)] proc macro.
//!
//! Run with: cargo run --example derive-tweenable

use eazy::Tweenable;

// Named fields struct.
#[derive(Clone, Copy, Debug, PartialEq, Tweenable)]
struct Position {
  x: f32,
  y: f32,
}

// Tuple struct.
#[derive(Clone, Copy, Debug, PartialEq, Tweenable)]
struct Vec2(f32, f32);

// Unit struct.
#[derive(Clone, Copy, Debug, PartialEq, Tweenable)]
struct Unit;

// Color with 4 components.
#[derive(Clone, Copy, Debug, PartialEq, Tweenable)]
struct Color {
  r: f32,
  g: f32,
  b: f32,
  a: f32,
}

fn main() {
  // Test named fields.
  let a = Position { x: 0.0, y: 0.0 };
  let b = Position { x: 100.0, y: 200.0 };
  let mid = a.lerp(b, 0.5);
  println!("Position lerp: {:?} -> {:?} @ 0.5 = {:?}", a, b, mid);
  assert_eq!(mid, Position { x: 50.0, y: 100.0 });

  // Test tuple struct.
  let a = Vec2(0.0, 0.0);
  let b = Vec2(100.0, 200.0);
  let mid = a.lerp(b, 0.5);
  println!("Vec2 lerp: {:?} -> {:?} @ 0.5 = {:?}", a, b, mid);
  assert_eq!(mid, Vec2(50.0, 100.0));

  // Test unit struct.
  let a = Unit;
  let b = Unit;
  let mid = a.lerp(b, 0.5);
  println!("Unit lerp: {:?} -> {:?} @ 0.5 = {:?}", a, b, mid);
  assert_eq!(mid, Unit);

  // Test color interpolation.
  let red = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
  };
  let blue = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
  };
  let purple = red.lerp(blue, 0.5);
  println!("Color lerp: {:?} -> {:?} @ 0.5 = {:?}", red, blue, purple);
  assert_eq!(
    purple,
    Color {
      r: 0.5,
      g: 0.0,
      b: 0.5,
      a: 1.0
    }
  );

  println!("\nAll derive macro tests passed!");
}
