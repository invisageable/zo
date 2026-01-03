use eazy::Curve;
use eazy::easing::oscillatory::bounce::OutBounce;

fn main() {
  for t in (0..=100).map(|x| x as f32 / 100.0) {
    let bounce = OutBounce.y(t);

    println!("reaction bounce: {:.3}", bounce);
  }
}
