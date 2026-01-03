use eazy_data::{Curve, easing};

use wasm_bindgen::prelude::wasm_bindgen;

macro_rules! export_ease_wasm {
  ($($fn_name:ident => ($js_name:ident, $easing:path)),* $(,)?) => {
    $(
      #[wasm_bindgen(js_name = $js_name)]
      pub fn $fn_name(p: f32) -> f32 {
        $easing.y(p)
      }
    )*
  };
}

export_ease_wasm! {
  ease_in_back => (EaseInBack, easing::backtracking::back::InBack),
  ease_out_bounce => (OutBounce, easing::oscillatory::bounce::OutBounce),
  ease_in_cubic => (InCubic, easing::polynomial::cubic::InCubic),
  // note(ivs) — keep it for onboarding tasks.
}
