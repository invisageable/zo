//! Binary codec for `Vec<UiCommand>` traffic between the
//! AOT-compiled exe and `libzo_runtime_native.dylib`.
//!
//! Encoder lives in `zo-codegen-arm` (compile-time, embeds
//! bytes in `__TEXT`); decoder lives in
//! `zo-runtime-native::ffi::_zo_run_native` (runtime,
//! reconstructs the `Vec`). Both are built from the same
//! workspace, so format-stability concerns that apply to
//! on-the-wire serialization don't apply here — `Cargo.lock`
//! pins postcard, encoder + decoder always agree.
//!
//! Postcard (varint, no_std-friendly) over the existing
//! `Serialize`/`Deserialize` derives on `UiCommand` and
//! transitive types. The cost of opting into postcard is
//! one ~10 KiB dep; the cost of hand-rolling per-variant
//! encode/decode arms is ~150 lines + maintenance churn
//! every time the protocol grows a variant.

use crate::UiCommand;

/// Codec error. Re-exported so consumers don't depend on
/// `postcard` directly — if the underlying format ever
/// changes, only this crate updates.
pub type CodecError = postcard::Error;

/// Encode a command stream to a self-contained byte buffer.
/// Format is postcard's standard varint encoding — the
/// caller treats it as opaque.
pub fn encode(cmds: &[UiCommand]) -> Result<Vec<u8>, CodecError> {
  postcard::to_stdvec(cmds)
}

/// Decode a byte buffer back into commands. Errors if `bytes`
/// was not produced by `encode` (or by an encoder built from
/// a different `UiCommand` shape — which the workspace lockstep
/// build prevents).
pub fn decode(bytes: &[u8]) -> Result<Vec<UiCommand>, CodecError> {
  postcard::from_bytes(bytes)
}

/// Encode any postcard payload (the tier-2 conditional branch
/// payload rides this; `encode` stays the command-stream entry).
pub fn encode_payload<T: serde::Serialize>(
  value: &T,
) -> Result<Vec<u8>, CodecError> {
  postcard::to_allocvec(value)
}

/// Decode any postcard payload — the sibling of
/// [`encode_payload`].
pub fn decode_payload<T: serde::de::DeserializeOwned>(
  bytes: &[u8],
) -> Result<T, CodecError> {
  postcard::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::{Attr, ElementTag, EventKind, PropValue, StyleScope, UiCommand};

  fn round_trip(cmds: Vec<UiCommand>) {
    let bytes = encode(&cmds).expect("encode");
    let back = decode(&bytes).expect("decode");

    assert_eq!(cmds, back);
  }

  #[test]
  fn round_trip_empty() {
    round_trip(vec![]);
  }

  #[test]
  fn round_trip_text_only() {
    round_trip(vec![UiCommand::Text("hello".into())]);
  }

  #[test]
  fn round_trip_button_with_event() {
    round_trip(vec![
      UiCommand::Element {
        tag: ElementTag::Button,
        attrs: vec![Attr::Prop {
          name: "id".into(),
          value: PropValue::Num(1),
        }],
        self_closing: false,
      },
      UiCommand::Text("-".into()),
      UiCommand::EndElement,
      UiCommand::Event {
        widget_id: "1".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
    ]);
  }

  #[test]
  fn round_trip_stylesheet_scoped_with_hash() {
    round_trip(vec![UiCommand::StyleSheet {
      css: ".btn { color: red }".into(),
      scope: StyleScope::Scoped,
      scope_hash: Some("_zo_a3f2".into()),
    }]);
  }

  #[test]
  fn round_trip_stylesheet_global_no_hash() {
    round_trip(vec![UiCommand::StyleSheet {
      css: "body { margin: 0 }".into(),
      scope: StyleScope::Global,
      scope_hash: None,
    }]);
  }

  #[test]
  fn round_trip_counter_template() {
    round_trip(vec![
      UiCommand::Element {
        tag: ElementTag::Button,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Text("-".into()),
      UiCommand::EndElement,
      UiCommand::Text("0".into()),
      UiCommand::Element {
        tag: ElementTag::Button,
        attrs: vec![],
        self_closing: false,
      },
      UiCommand::Text("+".into()),
      UiCommand::EndElement,
      UiCommand::Event {
        widget_id: "1".into(),
        event_kind: EventKind::Click,
        handler: "__closure_0".into(),
      },
      UiCommand::Event {
        widget_id: "2".into(),
        event_kind: EventKind::Click,
        handler: "__closure_1".into(),
      },
    ]);
  }

  #[test]
  fn decode_rejects_truncated_input() {
    let bytes = encode(&[UiCommand::Text("hello".into())]).unwrap();

    // Drop the last byte — payload no longer matches the
    // length prefix. Decode must surface an error rather
    // than panic / return garbage.
    let truncated = &bytes[..bytes.len() - 1];

    assert!(decode(truncated).is_err());
  }

  #[test]
  fn decode_rejects_empty_input() {
    // Empty bytes are not a valid encoding — postcard
    // needs at least one byte for the length-prefix
    // varint. The empty `Vec` round-trips through
    // `encode` to a single-byte `[0]`, not an empty
    // slice (`round_trip_empty` covers that path).
    assert!(decode(&[]).is_err());
  }
}
