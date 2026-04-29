fn main() {
  let width: usize = 31;
  let gens: usize = 15;
  let rule: [i32; 8] = [0, 1, 1, 1, 0, 1, 1, 0];

  let mut row: Vec<i32> = vec![0; width];
  row[width - 1] = 1;

  for _g in 0..gens {
    for p in 0..width {
      if row[p] == 1 {
        print!("#");
      } else {
        print!(".");
      }
    }
    println!();

    let mut next: Vec<i32> = vec![0; width];

    for k in 0..width {
      let left = if k > 0 { row[k - 1] } else { 0 };
      let center = row[k];
      let right = if k < width - 1 { row[k + 1] } else { 0 };
      let pattern = (left * 4 + center * 2 + right) as usize;
      next[k] = rule[pattern];
    }

    row = next;
  }
}
