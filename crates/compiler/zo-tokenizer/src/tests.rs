use crate::tokenizer;

use zo_reporter::Result;
use zo_session::session::Session;

#[test]
fn make_tokenizer() -> Result<()> {
  let mut session = Session::default();
  let tks1 = tokenizer::tokenize(&mut session, "")?;
  let tks2 = tokenizer::tokenize(&mut session, " ")?;
  let tks3 = tokenizer::tokenize(&mut session, "\t\n")?;

  assert_eq!(tks1.len(), 1);
  assert_eq!(tks2.len(), 1);
  assert_eq!(tks3.len(), 1);

  Ok(())
}

#[test]
fn scan_comments() -> Result<()> {
  let mut session = Session::default();

  let s1 =
    include_str!("../../zo-samples/atlas/tokens/program/comments/line.tks");

  let s2 =
    include_str!("../../zo-samples/atlas/tokens/program/comments/line-doc.tks");

  let tks1 = tokenizer::tokenize(&mut session, s1)?;
  let tks2 = tokenizer::tokenize(&mut session, s2)?;

  assert_eq!(tks1.len(), 1);
  assert_eq!(tks2.len(), 1);

  Ok(())
}
