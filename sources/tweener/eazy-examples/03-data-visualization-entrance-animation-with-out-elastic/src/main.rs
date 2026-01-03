use eazy::{Easing, ease};

fn main() {
  let duration = 1.0;
  let mut time = 0.0;
  let step = 0.05;

  while time <= duration {
    let scale = ease(Box::new(Easing::OutElastic), time / duration, 0.0, 1.0);

    println!("scale at {:.2}s = {:.4}", time, scale);

    time += step;
  }
}
