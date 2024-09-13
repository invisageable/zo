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

#[wasm_bindgen]
pub struct Repl {
  value: JsValue,
}

#[wasm_bindgen]
impl Repl {
  #[wasm_bindgen(constructor)]
  #[inline(always)]
  pub fn new() -> Self {
    Self {
      value: JsValue::from_str(""),
    }
  }

  #[wasm_bindgen]
  #[inline(always)]
  pub fn add_value(&mut self, value: JsValue) {
    self.value = value;
  }

  #[wasm_bindgen]
  #[inline]
  pub fn eval(&self) -> Result<JsValue, JsValue> {
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

    let tokens =
      tokenizer::tokenize(&mut session, &self.value.as_string().unwrap())
        .unwrap();

    let ast = parser::parse(&mut session, &tokens).unwrap();
    let ast = analyzer::analyze(&mut session, &ast).unwrap();
    let value = interpreter::interpret(&mut session, &ast).unwrap();

    Ok(JsValue::from_str(&format!("{value}")))
  }
}
