//! ```sh
//! cargo test -p swisskit-fmt --test fuzzing
//! ```

// use swisskit_fmt::pp;

// use proptest::prelude::{Just, Strategy};
// use proptest::proptest;

// fn arb_span() -> impl Strategy<Value = Span> {
//   (0usize..1_000)
//     .prop_flat_map(|start| {
//       (
//         Just(start),
//         start..start + 100,
//         1usize..100,
//         1usize..100,
//         1usize..500,
//         1usize..500,
//       )
//     })
//     .prop_map(|(start, end, start_line, end_line, start_col, end_col)| {
//       Span::of(start, end, start_line, end_line, start_col, end_col)
//     })
// }

// proptest! {
//   #[test]
//   fn merge_spans_should_not_panic(s1 in arb_span(), s2 in arb_span()) {
//     let _ = s1.merge(s2);
//   }

//   #[test]
//   fn merged_span_contains_both_spans(s1 in arb_span(), s2 in arb_span()) {
//     let merged = s1.merge(s2);

//     assert!(merged.start <= s1.start && merged.start <= s2.start);
//     assert!(merged.end >= s1.end && merged.end >= s2.end);
//   }
// }
