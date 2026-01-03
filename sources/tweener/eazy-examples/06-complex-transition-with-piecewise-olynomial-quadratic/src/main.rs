use eazy::Curve;
use eazy::interpolation::piecewize::polynomial::InPiecewizePolynomial;

fn main() {
  for time in (0..=100).map(|i| i as f32 / 100.0) {
    let transition_value = InPiecewizePolynomial.y(time);

    println!("time: {time:.2}, value: {transition_value:.3}");
  }
}
