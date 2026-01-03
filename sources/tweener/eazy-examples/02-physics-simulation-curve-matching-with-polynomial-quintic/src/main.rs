use eazy::Curve;
use eazy::easing::polynomial::quintic::InOutQuintic;

fn main() {
  for time in (0..=100).map(|x| x as f32 / 100.0) {
    let pos = InOutQuintic.y(time);

    println!("time: {:.2}, pos: {:.4}", time, pos);
  }
}
