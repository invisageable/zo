use regex::Regex;

pub fn format(text: impl AsRef<str>) -> String {
  let text = text.as_ref();
  let word_boundary_re = Regex::new(r"\b").unwrap();
  let mut result = String::new();

  // Split by word boundaries
  let mut last_index = 0;
  for mat in word_boundary_re.find_iter(text) {
    // Push the segment before the boundary
    if mat.start() > last_index {
      let segment = &text[last_index..mat.start()];
      result.push_str(&transform_segment(segment));
    }
    // Push the boundary itself (e.g., space or punctuation)
    result.push_str(mat.as_str());
    last_index = mat.end();
  }

  // Push the last segment after the last match
  if last_index < text.len() {
    let segment = &text[last_index..];
    result.push_str(&transform_segment(segment));
  }

  result
}

fn transform_segment(segment: &str) -> String {
  if segment.eq_ignore_ascii_case("zo") {
    return "zo".to_string();
  }
  if segment.eq_ignore_ascii_case("codelord") {
    return "codelord".to_string();
  }

  segment
    .chars()
    .map(|c| {
      if c.eq_ignore_ascii_case(&'i') {
        'i'
      } else {
        c.to_ascii_uppercase()
      }
    })
    .collect()
}
