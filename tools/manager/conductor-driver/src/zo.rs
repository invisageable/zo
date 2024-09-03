fn zo_compiler() {
  let root = env!("CARGO_MANIFEST_DIR");

  match Command::new(format!("{root}src/main.zo")).args(&[]) {
    Ok(mut process) => {
      let stdout = process.stdout.get_mut_ref();
      println!("output: {:s}", stdout.read_to_str().unwrap());

      let stderr = process.stderr.get_mut_ref();
      println!("err: {:s}", stderr.read_to_str().unwrap());
    }
    Err(err) => panic!("failed to conduct: {err}"),
  }
}
