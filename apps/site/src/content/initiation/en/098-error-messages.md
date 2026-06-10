# error messages

When a program does not compile, zo tells you what went wrong, where, and how to fix it. Pick the shape of that report with `--format`: a colored snippet for you, or structured data for a tool.

  ```zo
  fun main() {
    imu s: str = "hello" ++ 42;
  }
  ```

By default the compiler renders a human snippet to stderr — the offending line, a caret under each span, and the conflicting types in color.

  ```sh
  zo build greeting.zo
  ```

  ```text
  [E0304] Error • Type mismatch
     ╭─[ greeting.zo:2:25 ]
     │
   2 │   imu s: str = "hello" ++ 42;
     │                ───┬───    ─┬
     │                   ╰─────────── conflicts with this type `str`
     │                            ╰── incompatible type `int` here
  ```

## machine formats

An agent reads text differently than you do — it never skims and it is never overwhelmed by length. So zo offers two machine formats that carry the *full* diagnostic, not a terse summary. Both stream to stdout, leaving stderr for you.

`--format=json` emits one JSON object per diagnostic, one per line (NDJSON), flushed as each error is found.

  ```sh
  zo build greeting.zo --format=json
  ```

  ```json
  {"$schema":1,"id":"type-mismatch","code":"E0304","severity":"error","phase":"analyzer","message":"Type mismatch","fixes":[],"notes":["The types of both operands must be compatible"],"snippet":{"before":["fun main() {"],"lines":["  imu s: str = \"hello\" ++ 42;"],"after":["}"]},"span":{"file":"greeting.zo","byte_start":35,"byte_end":37,"line_start":2,"line_end":2,"col_start":25,"col_end":27},"secondary":{"file":"greeting.zo","byte_start":24,"byte_end":31,"line_start":2,"line_end":2,"col_start":14,"col_end":21},"primary_type":"int","secondary_type":"str"}
  ```

`--format=xml` emits one well-formed `<diagnostics>` document. The tag boundaries read as explicit structure — clean to drop straight into a prompt.

  ```sh
  zo build greeting.zo --format=xml
  ```

  ```xml
  <diagnostics schema="1">
    <diagnostic id="type-mismatch" code="E0304" severity="error" phase="analyzer">
      <message>Type mismatch</message>
      <fixes/>
      <notes>
        <note>The types of both operands must be compatible</note>
      </notes>
      <snippet>
        <before>
          <line>fun main() {</line>
        </before>
        <lines>
          <line>  imu s: str = "hello" ++ 42;</line>
        </lines>
        <after>
          <line>}</line>
        </after>
      </snippet>
      <span file="greeting.zo" byte_start="35" byte_end="37" line_start="2" line_end="2" col_start="25" col_end="27"/>
      <secondary file="greeting.zo" byte_start="24" byte_end="31" line_start="2" line_end="2" col_start="14" col_end="21"/>
      <primary_type>int</primary_type>
      <secondary_type>str</secondary_type>
    </diagnostic>
  </diagnostics>
  ```

The two machine formats are **isomorphic**: the same fields under the same names. A JSON key maps 1:1 onto the XML element or attribute of the same name, so a tool that reads one reads the other.

## the schema

Every diagnostic carries a stable identity and the data needed to act on it without re-parsing your source.

  - `id` — a frozen, kebab-case name (`type-mismatch`). Match on this, not the prose.
  - `code` — the display alias (`E0304`), derived from `id`.
  - `severity` — `error` or `warning`.
  - `phase` — where it surfaced: `tokenizer`, `parser`, `analyzer`, `codegen`, `runtime`.
  - `message` — the one-line headline.
  - `span` — the primary location: `file`, byte offsets, and 1-indexed `line`/`col` (columns count characters, so `é` advances one).
  - `secondary` — the conflicting location, when a diagnostic carries two spans.
  - `fixes` — machine-applicable edits, always an array. Each fix names a `kind` (`insert` / `replace` / `delete`), the replacement `text`, a `description`, and the exact span to edit. A tool auto-applying picks the first.
  - `notes` — attached context, always an array.
  - `snippet` — the source lines around the span (`before` / `lines` / `after`). Tune the radius with `--snippet-context N`; `0` turns it off.

`fixes` and `notes` are always present — empty rather than absent — so a consumer never needs a presence check. The same source over the same input renders byte-identical output, so a tool can diff two builds.

  ```zo
  -! ## the capstone.
  -!
  -!   - default `--format=human` paints a colored snippet to stderr.
  -!   - `--format=json` streams one NDJSON object per diagnostic to stdout.
  -!   - `--format=xml` emits one well-formed document to stdout.
  -!   - both machine formats share one frozen, isomorphic schema.
  -!   - match on the stable `id`, never on the prose `message`.
  -!   - `fixes` carry exact edits; `--snippet-context N` sets the source radius.
  ```