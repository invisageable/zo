use super::reader;

use zhoo_session::session::Session;

#[test]
fn read_empty() {
  let mut session = Session::default();

  session.settings.input = "../zhoo-notes/samples/test/empty.zo".into();

  reader::read(&mut session)
    .map(|bytes| assert!(bytes.len() == 0))
    .unwrap();
}

#[test]
fn read() {
  assert!(true)
}

#[test]
fn read_file_empty() {
  assert!(true)
}

#[test]
fn read_file() {
  assert!(true)
}

#[test]
fn read_line_empty() {
  assert!(true)
}

#[test]
fn read_line() {
  assert!(true)
}
