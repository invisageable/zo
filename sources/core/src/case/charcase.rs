pub mod endofcase;
pub mod groupcase;
pub mod identcase;
pub mod lowercase;
pub mod numbercase;
pub mod opcase;
pub mod punctuationcase;
pub mod quotecase;
pub mod spacecase;
pub mod uppercase;

#[macro_export]
macro_rules! is {
  (eof $rhs:expr) => {
    $crate::case::charcase::endofcase::is_eof($rhs)
  };
  (eol $rhs:expr) => {
    $crate::case::charcase::endofcase::is_eol($rhs)
  };
  (space $rhs:expr) => {
    $crate::case::charcase::spacecase::is_whitespace($rhs)
  };
  (quote $rhs:expr) => {
    $crate::case::charcase::quotecase::is_quote($rhs)
  };
  (quote_single $rhs:expr) => {
    $crate::case::charcase::quotecase::is_quote_single($rhs)
  };
  (quote_double $rhs:expr) => {
    $crate::case::charcase::quotecase::is_quote_double($rhs)
  };
  (number $rhs:expr) => {
    $crate::case::charcase::numbercase::is_number($rhs)
  };
  (number_zero $rhs:expr) => {
    $crate::case::charcase::numbercase::is_number_zero($rhs)
  };
  (number_continue $rhs:expr) => {
    $crate::case::charcase::numbercase::is_number_continue($rhs)
  };
  (number_hex $rhs:expr) => {
    $crate::case::charcase::numbercase::is_number_hex($rhs)
  };
  (ident $rhs:expr) => {
    $crate::case::charcase::identcase::is_ident($rhs)
  };
  (underscore $rhs:expr) => {
    $crate::case::charcase::identcase::is_underscore($rhs)
  };
  (lowercase $rhs:expr) => {
    $crate::case::charcase::lowercase::is_lowercase($rhs)
  };
  (uppercase $rhs:expr) => {
    $crate::case::charcase::uppercase::is_uppercase($rhs)
  };
}
