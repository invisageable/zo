use eazy::{Easing, ease};

fn main() {
  let duration = 2.0;
  let mut time = 0.0;
  let step = 1.0 / 60.0;

  while time <= duration {
    let progress = ease(Box::new(Easing::InOutBack), time / duration, 0.0, 1.0);

    println!("progress: {progress:.3}");

    time += step;
  }
}
