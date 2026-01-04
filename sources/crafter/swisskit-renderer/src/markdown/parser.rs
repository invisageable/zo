//! Markdown parsing utilities.
//!
//! This module provides utilities for parsing markdown text and
//! converting it to sanitized HTML.

use pulldown_cmark::{Options, Parser, html};

/// Create a markdown parser with default options.
///
/// Use this for rendering markdown to non-HTML backends (like egui).
/// For HTML output, use `to_html()` or `to_html_with_options()` instead.
pub fn create_parser<'input>(input: &'input str) -> Parser<'input> {
  Parser::new(input)
}

/// Create a markdown parser with custom options.
///
/// Use this for rendering markdown to non-HTML backends (like egui).
/// For HTML output, use `to_html_with_options()` instead.
pub fn create_parser_with_options<'input>(
  input: &'input str,
  options: Options,
) -> Parser<'input> {
  Parser::new_ext(input, options)
}

/// Convert markdown to sanitized HTML with default settings.
///
/// This is the recommended way to convert markdown to HTML as it
/// automatically sanitizes the output to prevent XSS attacks.
///
/// #### examples.
///
/// ```
/// use swisskit_renderer::markdown::parser::to_html;
///
/// let markdown = "[link](http://example.com/)";
/// let safe_html = to_html(markdown);
/// ```
pub fn to_html(markdown: &str) -> String {
  let mut unsafe_html = String::new();
  let parser = Parser::new(markdown);
  html::push_html(&mut unsafe_html, parser);

  ammonia::clean(&unsafe_html)
}

/// Convert markdown to sanitized HTML with custom parser options.
///
/// #### examples.
///
/// ```
/// use pulldown_cmark::Options;
/// use swisskit_renderer::markdown::parser::to_html_with_options;
///
/// let markdown = "# Heading\n\n| Table | Header |\n|-------|--------|\n| Cell  | Cell   |";
/// let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
/// let safe_html = to_html_with_options(markdown, options);
/// ```
pub fn to_html_with_options(markdown: &str, options: Options) -> String {
  let mut unsafe_html = String::new();
  let parser = Parser::new_ext(markdown, options);
  html::push_html(&mut unsafe_html, parser);

  ammonia::clean(&unsafe_html)
}

/// Convert markdown to sanitized HTML with custom ammonia settings.
///
/// This provides full control over both parsing and sanitization.
///
/// #### examples.
///
/// ```
/// use ammonia::Builder;
/// use pulldown_cmark::Options;
/// use swisskit_renderer::markdown::parser::to_html_custom;
///
/// let markdown = "[link](http://example.com/)";
/// let options = Options::empty();
/// let builder = Builder::default()
///   .add_tags(&["custom-tag"])
///   .add_tag_attributes("a", &["data-id"]);
///
/// let safe_html = to_html_custom(markdown, options, builder);
/// ```
pub fn to_html_custom(
  markdown: &str,
  options: Options,
  builder: ammonia::Builder,
) -> String {
  let mut unsafe_html = String::new();
  let parser = Parser::new_ext(markdown, options);
  html::push_html(&mut unsafe_html, parser);

  builder.clean(&unsafe_html).to_string()
}
