use crate::Compiler;

use zo_error::Severity;

use std::fs;

/// An error inside a loaded module must carry a file_id
/// that resolves to the module's source — not the entry
/// file. Verifies the file table + file_id wiring
/// end-to-end: `main.zo` loads `broken.zo`; the type
/// error lives in `broken.zo`; the renderer must see
/// `broken.zo`'s source, not `main.zo`'s.
#[test]
fn cross_module_error_carries_correct_file_id() {
  let dir = tempfile::tempdir().unwrap();
  let dir = dir.path();

  fs::write(dir.join("lib.zo"), "pub pack broken;\n").unwrap();

  fs::write(
    dir.join("broken.zo"),
    "pub fun oops() -> int {\n  \
       imu x: str = 42;\n  \
       return x;\n\
     }\n",
  )
  .unwrap();

  fs::write(
    dir.join("main.zo"),
    "load broken::*;\n\n\
     fun main() {\n  \
       imu v: int = oops();\n  \
       showln(\"{v}\");\n\
     }\n",
  )
  .unwrap();

  let main_path = dir.join("main.zo");
  let source = fs::read_to_string(&main_path).unwrap();

  let mut compiler = Compiler::new();

  let (_semantic, _tok, _par, _session, file_table) =
    compiler.analyze_source(&source, &main_path);

  let errors = compiler.reporter_errors();

  let hard_errors: Vec<_> = errors
    .iter()
    .filter(|e| matches!(e.severity(), Severity::Error))
    .collect();

  assert!(
    !hard_errors.is_empty(),
    "expected at least one error from broken.zo",
  );

  let error = hard_errors[0];
  let file_id = error.file_id();

  assert!(file_id.is_some(), "error must carry a file_id (not 0xFFFF)",);

  let fid = file_id.unwrap() as usize;

  assert!(
    fid < file_table.len(),
    "file_id {fid} out of range (table has {} entries)",
    file_table.len(),
  );

  let (path, file_source) = &file_table[fid];

  assert!(
    path.ends_with("broken.zo"),
    "error should point at broken.zo, got: {path:?}",
  );

  let span = error.span();

  assert!(
    (span.start as usize) < file_source.len(),
    "span.start ({}) must be within broken.zo's source \
     (len {})",
    span.start,
    file_source.len(),
  );
}
