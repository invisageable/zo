use zo_analyzer::analyzer;
use zo_interpreter::interpreter;
use zo_parser::parser;
use zo_session::backend::Backend;
use zo_session::session::SESSION;
use zo_session::settings::Settings;
use zo_tokenizer::tokenizer;

use wasm_bindgen::prelude::*;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// The reprensetation of a REPL.
#[wasm_bindgen]
pub struct Repl {}

#[wasm_bindgen]
impl Repl {
  /// Creates a new REPL.
  #[wasm_bindgen(constructor)]
  #[inline(always)]
  pub fn new() -> Self {
    Self {}
  }

  /// Evaluates a zo instruction.
  #[wasm_bindgen]
  #[inline]
  pub fn eval(&self, value: JsValue) -> Result<JsValue, JsValue> {
    // note(ivs) — the session should not be called here. actually we do not
    // have a smart evaluation, that mean each time we call `eval` the context
    // is erase by a new one. this is not the correct behavior.
    let session = std::sync::Arc::clone(&SESSION);
    let mut session = session.lock().unwrap();

    session.with_settings(Settings {
      backend: Backend::Zo,
      interactive: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        true,
      )),
      ..Default::default()
    });

    // drop(session);

    if let Some(value) = value.as_string() {
      let tokens = tokenizer::tokenize(&mut session, &value).unwrap();
      let ast = parser::parse(&mut session, &tokens).unwrap();
      let ast = analyzer::analyze(&mut session, &ast).unwrap();
      let value = interpreter::interpret(&mut session, &ast).unwrap();

      Ok(JsValue::from_str(&value.to_string()))
    } else {
      Err(JsValue::from_str("not a string."))
    }
  }
}
