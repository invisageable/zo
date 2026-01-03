#[macro_export]
macro_rules! is {
  // charcase.
  (eof $rhs:expr) => {
    $crate::charcase::endofcase::is_eof($rhs)
  };
  (eol $rhs:expr) => {
    $crate::charcase::endofcase::is_eol($rhs)
  };
  (group $rhs:expr) => {
    $crate::charcase::groupcase::is_group($rhs)
  };
  (space $rhs:expr) => {
    $crate::charcase::spacecase::is_space($rhs)
  };
  (quote $rhs:expr) => {
    $crate::charcase::quotecase::is_quote($rhs)
  };
  (quote_tick $ch:expr) => {
    $crate::charcase::quotecase::is_quote_backtick($ch)
  };
  (quote_single $rhs:expr) => {
    $crate::charcase::quotecase::is_quote_single($rhs)
  };
  (quote_double $rhs:expr) => {
    $crate::charcase::quotecase::is_quote_double($rhs)
  };
  (number $rhs:expr) => {
    $crate::charcase::numbercase::is_number($rhs)
  };
  (number_zero $rhs:expr) => {
    $crate::charcase::numbercase::is_number_zero($rhs)
  };
  (number_non_zero $rhs:expr) => {
    $crate::charcase::numbercase::is_number_non_zero($rhs)
  };
  (number_bin $rhs:expr) => {
    $crate::charcase::numbercase::is_number_bin($rhs)
  };
  (number_oct $rhs:expr) => {
    $crate::charcase::numbercase::is_number_oct($rhs)
  };
  (number_hex $rhs:expr) => {
    $crate::charcase::numbercase::is_number_hex($rhs)
  };
  (punctuation $rhs:expr) => {
    $crate::charcase::punctuationcase::is_punctuation($rhs)
  };
  (dot $rhs:expr) => {
    $crate::charcase::punctuationcase::is_dot($rhs)
  };
  (ident $rhs:expr) => {
    $crate::charcase::identcase::is_ident($rhs)
  };
  (ident_start $rhs:expr) => {
    $crate::charcase::identcase::is_ident_start($rhs)
  };
  (ident_continue $rhs:expr) => {
    $crate::charcase::identcase::is_ident_continue($rhs)
  };
  (underscore $rhs:expr) => {
    $crate::charcase::identcase::is_underscore($rhs)
  };
  (lowercase $rhs:expr) => {
    $crate::charcase::lowercase::is_lowercase($rhs)
  };
  (uppercase $rhs:expr) => {
    $crate::charcase::uppercase::is_uppercase($rhs)
  };

  (delim $rhs:expr) => {
    $crate::charcase::delimcase::is_delim($rhs)
  };

  // strcase
  (camel $rhs:expr) => {
    $crate::strcase::camelcase::is_camel_case($rhs)
  };
  (kebab $rhs:expr) => {
    $crate::strcase::kebabcase::is_kebab_case($rhs)
  };
  (pascal $rhs:expr) => {
    $crate::strcase::pascalcase::is_pascal_case($rhs)
  };
  (snake $rhs:expr) => {
    $crate::strcase::snakecase::is_snake_case($rhs)
  };
  (snake_screaming $rhs:expr) => {
    $crate::strcase::snakecase::is_snake_screaming_case($rhs)
  };
  (train $rhs:expr) => {
    $crate::strcase::traincase::is_train_case($rhs)
  };
}

#[macro_export]
macro_rules! to {
  // charcase.
  (lower_ascii $ch:expr) => {
    $crate::charcase::lowercase::to_lowercase_ascii($ch)
  };

  // strcase.
  (camel $rhs:expr) => {
    $crate::strcase::camelcase::to_camel_case($rhs)
  };
  (kebab $rhs:expr) => {
    $crate::strcase::kebabcase::to_kebab_case($rhs)
  };
  (pascal $rhs:expr) => {
    $crate::strcase::pascalcase::to_pascal_case($rhs)
  };
  (snake $rhs:expr) => {
    $crate::strcase::snakecase::to_snake_case($rhs)
  };
  (snake_screaming $rhs:expr) => {
    $crate::strcase::snakecase::to_snake_screaming_case($rhs)
  };
  (train $rhs:expr) => {
    $crate::strcase::traincase::to_train_case($rhs)
  };
  (singular $rhs:expr) => {
    $crate::strcase::singularcase::to_singular($rhs)
  };
  (plural $rhs:expr) => {
    $crate::strcase::pluralcase::to_plural($rhs)
  };
}
