use eazy::Curve;
use eazy::interpolation::polynomial::smoothstep::InSmooth;

fn main() {
  for time in (0..=100).map(|x| x as f32 / 100.0) {
    let scroll_position = InSmooth.y(time);

    println!("scroll: {scroll_position:.3}");
  }
}
