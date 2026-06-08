//! Serving the `public/` web bundle to the system browser.
//!
//! `zo run --target web` builds the bundle, then builds a [`Server`]
//! over its directory and calls [`Server::serve`], so the developer
//! never reaches for an external static-file server. The bundle is
//! self-contained (inlined CSS/JS, relative `assets/`), so a plain
//! HTTP/1.1 file server is enough — std only, no framework, in keeping
//! with owning the stack.

use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;

/// The loopback address the preview server binds. Port `0` lets the OS
/// hand out a free ephemeral port, so concurrent runs never collide.
const LOOPBACK: (&str, u16) = ("127.0.0.1", 0);

/// Plain-text MIME type, used for error bodies.
const TEXT: &str = "text/plain; charset=utf-8";

/// The `404` response body.
const NOT_FOUND: &[u8] = b"404 not found";

/// Whether [`Server::serve`] opens the system browser on start.
#[derive(Clone, Copy, Default)]
pub enum Browsering {
  /// Open the browser at the served URL — the `zo run --target web`
  /// default.
  #[default]
  Yes,
  /// Serve only; leave the browser alone.
  No,
}

/// A static file server rooted at one `public/` bundle directory.
#[derive(Clone)]
pub struct Server {
  /// The bundle directory every request resolves against.
  root: PathBuf,
  /// Whether starting the server opens the browser.
  browsering: Browsering,
}

impl Server {
  /// A server serving the bundle at `dir`, opening the browser on
  /// start.
  pub fn new(dir: &Path) -> Self {
    Self {
      root: dir.to_path_buf(),
      browsering: Browsering::default(),
    }
  }

  /// Override whether starting the server opens the browser.
  pub fn with_browsering(mut self, browsering: Browsering) -> Self {
    self.browsering = browsering;
    self
  }

  /// Bind localhost, open the browser, and serve requests until the
  /// process is killed (Ctrl-C) — the same lifetime as the in-process
  /// window paths. Each connection is handled on its own thread so a
  /// slow request can't block the page.
  pub fn serve(&self) -> io::Result<()> {
    let listener = TcpListener::bind(LOOPBACK)?;
    let url = format!("http://127.0.0.1:{}/", listener.local_addr()?.port());

    println!("zo web — serving {} at {url}", self.root.display());
    println!("zo web — press Ctrl-C to stop.");

    if let Browsering::Yes = self.browsering {
      Self::open_in_browser(&url);
    }

    for stream in listener.incoming() {
      match stream {
        Ok(stream) => {
          let server = self.clone();

          thread::spawn(move || {
            if let Err(error) = server.respond_to(stream) {
              eprintln!("zo web — request error: {error}");
            }
          });
        }
        Err(error) => eprintln!("zo web — accept error: {error}"),
      }
    }

    Ok(())
  }

  /// Read one request off the socket and write the response, closing
  /// the connection after.
  fn respond_to(&self, mut stream: TcpStream) -> io::Result<()> {
    let reader = BufReader::new(stream.try_clone()?);

    self.handle(reader, &mut stream)
  }

  /// Map the request path into the bundle and write the file (or a
  /// `404`). Generic over the transport so a test can drive one request
  /// without a socket. Only the request line matters for a static GET
  /// server; the rest is ignored.
  fn handle<R: BufRead, W: Write>(
    &self,
    mut reader: R,
    writer: &mut W,
  ) -> io::Result<()> {
    let mut request_line = String::new();

    reader.read_line(&mut request_line)?;

    // "GET /path HTTP/1.1" — the path is the second whitespace field.
    let request_path = request_line.split_whitespace().nth(1).unwrap_or("/");

    match self.resolve(request_path) {
      Some(path) => match fs::read(&path) {
        Ok(body) => {
          Self::reply(writer, 200, "OK", Self::content_type(&path), &body)
        }
        Err(_) => Self::reply(writer, 404, "Not Found", TEXT, NOT_FOUND),
      },
      None => Self::reply(writer, 404, "Not Found", TEXT, NOT_FOUND),
    }
  }

  /// Map a request path to a file inside the bundle, or `None` when it
  /// escapes the bundle or is missing. `/` serves `index.html`. Path
  /// traversal (`..`, absolute prefixes) is rejected by keeping only
  /// `Normal` components, and a final canonicalized-containment check
  /// guards against symlink escapes.
  fn resolve(&self, request_path: &str) -> Option<PathBuf> {
    let path = request_path.split(['?', '#']).next().unwrap_or("/");
    let path = if path == "/" { "/index.html" } else { path };

    let mut relative = PathBuf::new();

    for component in Path::new(path).components() {
      match component {
        Component::Normal(part) => relative.push(part),
        Component::RootDir | Component::CurDir => {}
        Component::ParentDir | Component::Prefix(_) => return None,
      }
    }

    let full = self.root.join(relative).canonicalize().ok()?;

    full
      .starts_with(self.root.canonicalize().ok()?)
      .then_some(full)
  }

  /// Write a full HTTP/1.1 response and close the connection.
  fn reply<W: Write>(
    writer: &mut W,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
  ) -> io::Result<()> {
    let header = format!(
      "HTTP/1.1 {status} {reason}\r\n\
       Content-Type: {content_type}\r\n\
       Content-Length: {}\r\n\
       Connection: close\r\n\
       \r\n",
      body.len(),
    );

    writer.write_all(header.as_bytes())?;
    writer.write_all(body)?;
    writer.flush()
  }

  /// The MIME type for a file, keyed off its extension. Covers the
  /// asset kinds a zo web bundle emits; anything else is served as
  /// opaque bytes.
  fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
      Some("html") => "text/html; charset=utf-8",
      Some("css") => "text/css; charset=utf-8",
      Some("js" | "mjs") => "text/javascript; charset=utf-8",
      Some("json") => "application/json; charset=utf-8",
      Some("svg") => "image/svg+xml",
      Some("png") => "image/png",
      Some("jpg" | "jpeg") => "image/jpeg",
      Some("gif") => "image/gif",
      Some("webp") => "image/webp",
      Some("ico") => "image/x-icon",
      Some("woff2") => "font/woff2",
      Some("woff") => "font/woff",
      Some("ttf") => "font/ttf",
      Some("otf") => "font/otf",
      Some("wasm") => "application/wasm",
      Some("txt") => TEXT,
      _ => "application/octet-stream",
    }
  }

  /// Open `url` in the system browser. A failure is non-fatal — the
  /// server keeps running and the URL is already printed.
  fn open_in_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let command = Command::new("open").arg(url).spawn();

    #[cfg(target_os = "windows")]
    let command = Command::new("cmd").args(["/C", "start", "", url]).spawn();

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let command = Command::new("xdg-open").arg(url).spawn();

    if let Err(error) = command {
      eprintln!("zo web — could not open browser ({error}); visit {url}");
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Server;

  use std::io::Cursor;
  use std::path::{Path, PathBuf};

  /// The committed `samples/serve/` bundle the server tests resolve
  /// against.
  fn bundle() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/serve")
  }

  /// Drive one request through `handle` and return the raw response.
  fn request(path: &str) -> String {
    let server = Server::new(&bundle());
    let line = format!("GET {path} HTTP/1.1\r\n");
    let mut response = Vec::new();

    server
      .handle(Cursor::new(line.into_bytes()), &mut response)
      .expect("handle should not fail");

    String::from_utf8(response).expect("response is UTF-8 in these tests")
  }

  #[test]
  fn root_serves_index_html() {
    let response = request("/");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: text/html; charset=utf-8"));
    assert!(response.contains("hello from the bundle"));
  }

  #[test]
  fn named_file_serves_with_its_mime() {
    let response = request("/style.css");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("Content-Type: text/css; charset=utf-8"));
    assert!(response.contains("rebeccapurple"));
  }

  #[test]
  fn query_string_is_ignored() {
    let response = request("/style.css?v=2");

    assert!(response.starts_with("HTTP/1.1 200 OK"));
    assert!(response.contains("rebeccapurple"));
  }

  #[test]
  fn missing_file_is_404() {
    let response = request("/nope.js");

    assert!(response.starts_with("HTTP/1.1 404 Not Found"));
    assert!(response.contains("404 not found"));
  }

  #[test]
  fn path_traversal_is_rejected() {
    // `..` must never escape the bundle into the source tree.
    assert!(Server::new(&bundle()).resolve("/../Cargo.toml").is_none());
    assert!(request("/../../etc/passwd").starts_with("HTTP/1.1 404"));
  }

  #[test]
  fn content_type_maps_known_extensions() {
    let cases = [
      ("a.html", "text/html; charset=utf-8"),
      ("a.css", "text/css; charset=utf-8"),
      ("a.js", "text/javascript; charset=utf-8"),
      ("a.png", "image/png"),
      ("a.woff2", "font/woff2"),
      ("a.bin", "application/octet-stream"),
    ];

    for (file, expected) in cases {
      assert_eq!(Server::content_type(Path::new(file)), expected);
    }
  }
}
