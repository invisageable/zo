//! eazy-bevy example — Bouncing squares with different easings.

use bevy::{color::palettes::css::*, prelude::*};
use eazy_bevy::{Animator, Easing, EazyPlugin, Tween};

fn main() {
  App::new()
    .add_plugins(DefaultPlugins.set(WindowPlugin {
      primary_window: Some(Window {
        resolution: [800.0, 600.0].into(),
        title: "eazy-bevy example".into(),
        ..default()
      }),
      ..default()
    }))
    .add_plugins(EazyPlugin)
    .add_systems(Startup, setup)
    .add_systems(Update, (animate_y, animate_scale, restart_on_complete))
    .run();
}

/// Marker for Y position animation.
#[derive(Component)]
struct AnimateY;

/// Marker for scale animation.
#[derive(Component)]
struct AnimateScale;

fn setup(
  mut commands: Commands,
  mut meshes: ResMut<Assets<Mesh>>,
  mut materials: ResMut<Assets<ColorMaterial>>,
) {
  commands.spawn(Camera2d::default());

  // Define easings to showcase.
  let easings = [
    (Easing::Linear, RED),
    (Easing::InOutQuadratic, ORANGE),
    (Easing::OutBounce, YELLOW),
    (Easing::OutElastic, GREEN),
    (Easing::InOutBack, AQUA),
  ];

  let spacing = 120.0;
  let start_x = -((easings.len() as f32 - 1.0) * spacing) / 2.0;

  for (i, (easing, color)) in easings.iter().enumerate() {
    let x = start_x + i as f32 * spacing;

    commands.spawn((
      Mesh2d(meshes.add(Rectangle::new(60.0, 60.0))),
      MeshMaterial2d(materials.add(ColorMaterial::from_color(*color))),
      Transform::from_xyz(x, -200.0, 0.0),
      // Y position animator.
      Animator::new(
        Tween::to(-200.0_f32, 200.0)
          .duration(2.0)
          .easing(easing.clone())
          .build(),
      )
      .playing(),
      AnimateY,
    ));
  }

  // Spawn a pulsing square in the center.
  commands.spawn((
    Mesh2d(meshes.add(Rectangle::new(80.0, 80.0))),
    MeshMaterial2d(materials.add(ColorMaterial::from_color(WHITE))),
    Transform::from_xyz(0.0, 0.0, 1.0),
    // Scale animator using [f32; 3] for Vec3-like behavior.
    Animator::new(
      Tween::to([0.5_f32, 0.5, 0.5], [1.5, 1.5, 1.5])
        .duration(1.0)
        .easing(Easing::InOutSine)
        .build(),
    )
    .playing(),
    AnimateScale,
  ));
}

/// Apply Y position from animator to transform.
fn animate_y(
  mut query: Query<(&mut Transform, &Animator<f32>), With<AnimateY>>,
) {
  for (mut transform, animator) in &mut query {
    transform.translation.y = animator.value();
  }
}

/// Apply scale from animator to transform.
fn animate_scale(
  mut query: Query<(&mut Transform, &Animator<[f32; 3]>), With<AnimateScale>>,
) {
  for (mut transform, animator) in &mut query {
    let [x, y, z] = animator.value();
    transform.scale = Vec3::new(x, y, z);
  }
}

/// Restart animations when they complete (creates loop).
fn restart_on_complete(
  mut y_query: Query<&mut Animator<f32>, With<AnimateY>>,
  mut scale_query: Query<&mut Animator<[f32; 3]>, With<AnimateScale>>,
) {
  for mut animator in &mut y_query {
    if animator.is_complete() {
      animator.restart();
      animator.play();
    }
  }

  for mut animator in &mut scale_query {
    if animator.is_complete() {
      animator.reverse();
      animator.restart();
      animator.play();
    }
  }
}
