const WIDTH: i64 = 800;
const HEIGHT: i64 = 600;
const MAX_ITER: i64 = 100;

fn main() {
  let width_f = WIDTH as f64;
  let height_f = HEIGHT as f64;

  let mut checksum: i64 = 0;

  for y in 0..HEIGHT {
    for x in 0..WIDTH {
      let cr = x as f64 / width_f * 3.0 - 2.0;
      let ci = y as f64 / height_f * 2.24 - 1.12;

      let mut zr = 0.0_f64;
      let mut zi = 0.0_f64;
      let mut iter: i64 = 0;

      while zr * zr + zi * zi <= 4.0 && iter < MAX_ITER {
        let temp = zr * zr - zi * zi + cr;
        zi = 2.0 * zr * zi + ci;
        zr = temp;
        iter += 1;
      }

      checksum += iter;
    }
  }

  println!("{checksum}");
}