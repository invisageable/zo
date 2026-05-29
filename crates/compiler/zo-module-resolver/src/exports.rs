use zo_error::{Error, ErrorKind};
use zo_interner::{Interner, Symbol};
use zo_reporter::report_error;
use zo_sir::{Insn, Sir};
use zo_span::Span;
use zo_token::LiteralStore;
use zo_tree::{NodeHeader, NodeValue, Tree};
use zo_ty::TyId;
use zo_value::{FunDef, Local, Pubness, ValueId};

use rustc_hash::FxHashMap;

use std::path::PathBuf;

/// `true` when `name` matches a selective-load filter.
///
/// Equality match (`name == filter`) is the obvious
/// case — `load M::(Json);` picking up the `Json`
/// struct. The prefix case (`name` starting with
/// `<filter>::`) lets a selective load of a type
/// (`load M::(Json);` or the bare `load M::Json;`)
/// also pull in every `apply <filter> { fun ... }`
/// method, whose names are mangled as `<filter>::<m>`.
/// Without this, `Json::parse` would be filtered out
/// and user code calling `Json::parse(ws)` would fail
/// with `UndefinedFunction` even though the user
/// explicitly named the type they wanted.
fn matches_selective(name: &str, filter: &str) -> bool {
  if name == filter {
    return true;
  }

  if let Some(rest) = name.strip_prefix(filter)
    && rest.starts_with("::")
  {
    return true;
  }

  false
}

/// An `abstract` definition — method signatures, no
/// bodies. Lives here so [`ImportedSymbols`] (the
/// cross-module symbol bundle) can carry it without
/// pulling `zo-executor` in upstream.
#[derive(Clone)]
pub struct AbstractDef {
  pub methods: Vec<AbstractMethod>,
  /// Aggregate of each method's `dyn_safe`. `false`
  /// blocks `any <Abstract>` resolution at the
  /// annotation site — the vtable calling convention
  /// can't carry `Self` outside the receiver.
  pub dyn_safe: bool,
  /// Source span of the `abstract` keyword.
  pub span: Span,
}

/// A single method signature in an abstract definition.
#[derive(Clone)]
pub struct AbstractMethod {
  pub name: Symbol,
  pub params: Vec<(Symbol, TyId)>,
  pub return_ty: TyId,
  /// `false` iff `Self` appears in a non-receiver
  /// param or as the return type. Caught at parse
  /// time while the literal `Token::SelfUpper` is
  /// still visible.
  pub dyn_safe: bool,
}

/// One `apply Abstract for Type { ... }` registration.
/// Carries everything an importer needs to fold the impl
/// into its own dispatch table — and everything the
/// coherence check needs to surface a precise duplicate-
/// impl diagnostic.
///
/// Keyed by `(abstract_name, target_type)` in
/// `abstract_impls`; the value here records the methods
/// the apply block introduced plus the defining-site
/// location so a downstream collision (two modules each
/// implementing `Eq` for the same struct) can point at
/// both sites in the error.
#[derive(Clone, Debug)]
pub struct AbstractImpl {
  /// Fully-mangled method symbols (e.g. `Point::eq`) the
  /// apply block contributed. Captured by snapshotting
  /// `funs.len()` before the body walks and slicing
  /// after — never by name-prefix scan, which would
  /// contaminate the set with sibling `apply Point` /
  /// `apply Show for Point` entries.
  pub methods: Vec<Symbol>,
  /// Span of the `apply` keyword in the defining module's
  /// source. Used as the anchor for the duplicate-impl
  /// diagnostic.
  pub defined_at: Span,
  /// Filesystem path of the defining module — span alone
  /// is byte offsets with no module identity, so the
  /// coherence error needs this to render "first at
  /// core/foo.zo:12, second at user.zo:5".
  pub defining_module: PathBuf,
  /// Visibility of the enclosing pack. `extract_exports`
  /// only ships entries with `Pubness::Yes` so a private
  /// `pack foo;` stays a black box even if its body
  /// implements public abstracts. Mirrors the orphan-rule
  /// shape Rust enforces at the crate boundary.
  pub pubness: Pubness,
  /// Pre-interned `__zo_vtable_<Abstract>__<ConcreteType>`
  /// symbol. Minted at apply-block time so codegen can
  /// reference the vtable without a mutable interner.
  pub vtable_sym: Symbol,
}

/// An exported compile-time constant from a module.
#[derive(Clone, Debug)]
pub struct ExportedVar {
  /// The name of the constant (re-interned).
  pub name: Symbol,
  /// The type of the constant.
  pub ty_id: TyId,
  /// The initializer value (if compile-time known).
  pub init: Option<ValueId>,
  /// Pack that declared this constant.
  pub owning_pack: Option<Symbol>,
  /// Portable literal value for cross-module `val` import.
  pub literal: Option<ExportedLiteral>,
}

/// Exported enum definition for cross-module import. Carries
/// raw variant data instead of TyChecker-internal IDs so the
/// importing executor can re-intern into its own TyChecker.
#[derive(Clone)]
pub struct ExportedEnum {
  pub name: Symbol,
  pub variants: Vec<(Symbol, u32, Vec<TyId>)>,
}

/// Exported struct definition for cross-module import.
#[derive(Clone)]
pub struct ExportedStruct {
  pub name: Symbol,
  pub ty_id: TyId,
  pub fields: Vec<(Symbol, TyId, bool)>,
}

/// Exported compile-time constant (`val`).
pub struct ExportedConst {
  pub name: Symbol,
  pub ty_id: TyId,
  pub value: ValueId,
}

/// Exported generic apply-block body — the tree subrange the
/// importing executor needs to re-execute when it encounters
/// a call to `arr_$::<method>` (or any generic mangled
/// symbol). Carries everything `reexecute_generic_instantiations`
/// reads off `self.tree`, so the importer can splice into its
/// own tree and register a fresh `generic_tree_ranges` entry
/// pointing at the spliced offset.
///
/// @note — `nodes[0]` is the `Fun` introducer; the rest of
/// the slice walks postorder through the body up to the
/// closing `}`. Child indices inside the nodes are *absolute*
/// in the defining module's tree — the splice path adds the
/// importer's pre-splice `nodes.len()` to rebase them.
#[derive(Clone)]
pub struct ExportedGenericBody {
  /// Mangled apply-level symbol, e.g. `arr_$::first`.
  pub name: Symbol,
  /// Body subtree nodes (postorder), cloned from the
  /// defining module's `Tree::nodes`.
  pub nodes: Vec<NodeHeader>,
  /// Spans parallel to `nodes`.
  pub spans: Vec<Span>,
  /// Sparse node values keyed by `(absolute_index_in_nodes,
  /// value)`. The importer rebases the index when splicing.
  pub node_values: Vec<(u32, NodeValue)>,
  /// Literal payloads keyed by absolute defining-module
  /// node index. Each entry pairs with a `NodeValue::Literal`
  /// in `node_values` at the same index. At splice time the
  /// importer pushes the payload into its own `LiteralStore`
  /// and rewrites the rebased value's index to point at the
  /// new slot.
  pub literal_payloads: Vec<(u32, ExportedLiteral)>,
  /// First node index in the defining module's tree — used
  /// by the importer to compute the rebase offset.
  pub origin_start: u32,
  /// Apply-level type params (e.g. `[$T]`) so the importer
  /// can mirror `apply_type_params` against the spliced
  /// symbol.
  pub type_params: Vec<Symbol>,
  /// Apply-context symbol — the prefix the re-exec pass
  /// reads to look up `apply_type_params`. For
  /// `apply []$T { fun first(self) ... }` this is `arr_$`;
  /// the body's mangled name is `arr_$::first`.
  pub apply_context: Symbol,
}

/// Post-splice metadata for one generic body — the executor
/// reads this in `with_imports` to register
/// `generic_tree_ranges` + `apply_type_params` after the
/// pre-pass has already mutated the shared `Tree` /
/// `LiteralStore`.
#[derive(Clone, Debug)]
pub struct SplicedGenericBody {
  pub name: Symbol,
  /// Range in the importer's tree where the splice landed,
  /// `(start_inclusive, end_exclusive)`.
  pub range: (u32, u32),
  pub apply_context: Symbol,
  pub type_params: Vec<Symbol>,
}

/// Imported module symbols to pre-load into the executor.
/// Built by the compiler driver (one per loaded module's
/// transitive scope) and handed straight to
/// `Executor::with_imports`.
///
/// Lives in `zo-module-resolver` rather than `zo-analyzer`
/// so the executor can consume it by value without
/// inverting the layering (executor sits below analyzer).
#[derive(Clone, Default)]
pub struct ImportedSymbols {
  /// The function definitions from loaded modules.
  pub funs: Vec<FunDef>,
  /// Constants from loaded modules.
  pub vars: Vec<Local>,
  /// Portable literal values parallel to `vars`.
  pub var_literals: Vec<Option<ExportedLiteral>>,
  /// Enum definitions from loaded modules (raw variant data
  /// for re-interning in the executor's own TyChecker).
  pub enums: Vec<ExportedEnum>,
  /// Struct definitions from loaded modules (forward-
  /// registered so the prescan can resolve imported types
  /// in function signatures).
  pub structs: Vec<ExportedStruct>,
  /// Abstract definitions from loaded modules.
  pub abstract_defs: FxHashMap<Symbol, AbstractDef>,
  /// `(Abstract, Type) -> AbstractImpl` rolled-up across
  /// every transitively-imported module's exports. The
  /// compiler driver folds these in via
  /// `fold_imports_into`, raising `DuplicateAbstractImpl`
  /// against the two defining-module spans whenever a
  /// collision shows up.
  pub abstract_impls: FxHashMap<(Symbol, Symbol), AbstractImpl>,
  /// Generic apply-block bodies recorded by upstream
  /// modules. The compiler runs `splice_generic_bodies`
  /// over this vec against the importing module's `Tree`
  /// / `LiteralStore` right before constructing the
  /// `Analyzer`, then stores the post-splice metadata in
  /// [`Self::generic_bodies`].
  pub exported_generic_bodies: Vec<ExportedGenericBody>,
  /// Post-splice metadata for generic apply-block bodies
  /// the compiler pre-pass already wove into the shared
  /// `Tree` / `LiteralStore`. The executor registers each
  /// entry in `generic_tree_ranges` + `apply_type_params`
  /// at `with_imports` time; the body nodes themselves are
  /// already live in `Tree`.
  pub generic_bodies: Vec<SplicedGenericBody>,
}

impl ImportedSymbols {
  /// `true` when this scope carries nothing the executor
  /// needs to register. Drives the `with_imports` skip in
  /// the analyzer's entry path.
  ///
  /// @note — checks `generic_bodies` too. A scope carrying
  /// only spliced generic bodies (no funs / vars / enums /
  /// abstracts) would otherwise skip `with_imports`,
  /// leaving `splice_boundary` + `generic_tree_ranges`
  /// unpopulated; the main walk would then step past EOF
  /// into the appended `$T` nodes and raise spurious
  /// `UndefinedTypeParam` on unrelated user code.
  pub fn is_empty(&self) -> bool {
    self.funs.is_empty()
      && self.vars.is_empty()
      && self.enums.is_empty()
      && self.structs.is_empty()
      && self.abstract_defs.is_empty()
      && self.abstract_impls.is_empty()
      && self.exported_generic_bodies.is_empty()
      && self.generic_bodies.is_empty()
  }
}

/// Splices a batch of `ExportedGenericBody` payloads into
/// the importer's `Tree` and `LiteralStore`. Returns one
/// [`SplicedGenericBody`] entry per body the executor needs
/// to register against the standard mono re-execute path.
///
/// The mutation happens here (compiler pre-pass) so the
/// executor can keep `Tree` / `LiteralStore` as `&` refs
/// during execution. Each body's nodes/spans/values land
/// at the tail of `tree`; literal payloads land at the tail
/// of `literals`; `NodeValue::Literal(i)` indices and node
/// `child_start` fields get rebased onto the new offsets.
///
/// @note — bails on one body without bringing down the
/// others when its spliced range would overflow
/// `NodeHeader.child_start` (`u16`). v1 cap is `u16::MAX`;
/// widening is a separate refactor.
pub fn splice_generic_bodies(
  tree: &mut Tree,
  literals: &mut LiteralStore,
  bodies: Vec<ExportedGenericBody>,
) -> Vec<SplicedGenericBody> {
  let mut out: Vec<SplicedGenericBody> = Vec::with_capacity(bodies.len());

  for body in bodies {
    if let Some(meta) = splice_one(tree, literals, body) {
      out.push(meta);
    }
  }

  out
}

fn splice_one(
  tree: &mut Tree,
  literals: &mut LiteralStore,
  body: ExportedGenericBody,
) -> Option<SplicedGenericBody> {
  let splice_start = tree.nodes.len();
  let splice_end = splice_start + body.nodes.len();

  // u32 cap on `NodeHeader.child_start` — past this the
  // rebase below silently truncates and the spliced bodies'
  // parent→child edges point at unrelated nodes. Emit a real
  // diagnostic anchored at the body's first span so the user
  // sees which module hit the cap, not just a downstream
  // "Undefined variable" cascade. 4 billion nodes is far
  // beyond any real program; the guard stays as a canary.
  if splice_end > u32::MAX as usize {
    let span = body.spans.first().copied().unwrap_or_default();
    report_error(Error::new(ErrorKind::CrossModuleGenericTooLarge, span));

    return None;
  }

  // Snapshots for end-of-splice invariant checks. Catches
  // the parallel-length handshake breaking the moment a
  // second caller appears (phase 4+).
  let pre_nodes_len = tree.nodes.len();
  let pre_spans_len = tree.spans.len();

  // `child_start` is absolute in the defining tree; shift
  // by `splice_start - origin_start` so it lands at the
  // same relative offset inside the spliced range.
  let offset = splice_start as i64 - body.origin_start as i64;

  // Replay literal payloads into the importer's
  // `LiteralStore`, mapping orig→new index so the
  // `NodeValue::Literal` walk below can rewrite each index
  // in one pass.
  let mut literal_remap: FxHashMap<u32, u32> = FxHashMap::default();

  for (orig_node_idx, payload) in &body.literal_payloads {
    let new_lit_idx = match payload {
      ExportedLiteral::Int(v) => literals.push_int(*v),
      ExportedLiteral::Float(v) => literals.push_float(*v),
      ExportedLiteral::Char(v) => literals.push_char(*v),
      ExportedLiteral::StringSym(sym) => literals.push_string_symbol(*sym),
      ExportedLiteral::Bytes(v) => literals.push_bytes(*v),
    };

    literal_remap.insert(*orig_node_idx, new_lit_idx);
  }

  // Clone nodes with rebased `child_start`. Postorder
  // children always precede their parent inside the body
  // subrange, so `+ offset` keeps every parent->child edge
  // pointing inside the spliced block.
  for node in &body.nodes {
    let mut clone = *node;

    // FLAG_HAS_EXPLICIT_CHILDREN (sidecar `child_indices`
    // sparse table) is declared but unused today. If a
    // future parser path starts setting it, the splice
    // would need a parallel `child_indices` rebase — bail
    // explicitly until that lands, rather than silently
    // splicing wrong references.
    if clone.has_explicit_children() {
      let span = body.spans.first().copied().unwrap_or_default();
      report_error(Error::new(ErrorKind::CrossModuleGenericTooLarge, span));

      // Rewind any partial pushes from this body so the
      // tree stays in a clean post-splice state.
      tree.nodes.truncate(pre_nodes_len);
      tree.spans.truncate(pre_spans_len);

      return None;
    }

    if clone.child_count > 0 {
      let new_child_start = clone.child_start as i64 + offset;

      // Debug-only canary against a misrecorded body whose
      // internal child indices lie outside the body's own
      // subrange. `child_start` is u32, so the only real
      // ceiling is u32::MAX nodes.
      debug_assert!(
        new_child_start >= 0 && new_child_start <= u32::MAX as i64,
        "child_start rebase out of u32 range"
      );

      clone.child_start = new_child_start as u32;
    }

    tree.nodes.push(clone);
  }

  for span in &body.spans {
    tree.spans.push(*span);
  }

  // Push `node_values` with rebased indices and rewritten
  // literal slots. `value_map` stays sorted because
  // `splice_start` is always above any pre-existing entry.
  for (orig_node_idx, value) in &body.node_values {
    let new_node_idx = (*orig_node_idx as i64 + offset) as u32;
    let new_value = match value {
      NodeValue::Literal(_) => {
        let new_lit_idx =
          literal_remap.get(orig_node_idx).copied().unwrap_or(0);

        NodeValue::Literal(new_lit_idx)
      }
      other => *other,
    };

    tree.attach_value_tail(new_node_idx, new_value);
  }

  // Parallel-length / sortedness invariants — debug only
  // so release stays branchless. The lock-in here catches
  // the moment phase 4 adds a second caller and forgets
  // any of the three arrays.
  debug_assert_eq!(
    tree.nodes.len(),
    tree.spans.len(),
    "splice broke nodes/spans parallel length"
  );
  debug_assert_eq!(
    tree.nodes.len(),
    splice_end,
    "splice grew nodes beyond the announced range"
  );
  debug_assert!(
    tree.value_map_is_sorted(),
    "splice broke value_map sort invariant"
  );

  Some(SplicedGenericBody {
    name: body.name,
    range: (splice_start as u32, splice_end as u32),
    apply_context: body.apply_context,
    type_params: body.type_params,
  })
}

/// Literal payload snapshotted at body-record time. The
/// defining module's `LiteralStore` is consumed by the time
/// `with_imports` runs, so the value rides along with the
/// body or it's lost.
///
/// @note — v1 covers `Int` / `Float` / `Char` / `Str` /
/// `Bytes`. Interpolated strings and regex literals carry
/// per-store range tables and are rejected at record time
/// with `ErrorKind::Unsupported`.
#[derive(Clone, Debug)]
pub enum ExportedLiteral {
  Int(u64),
  Float(f64),
  Char(u32),
  /// String / identifier symbol from the shared session
  /// `Interner`. The symbol value survives unchanged; only
  /// the `LiteralStore` slot it sits at changes.
  StringSym(Symbol),
  Bytes(u8),
}

/// Exported symbols from a compiled module.
pub struct ModuleExports {
  /// The function definitions.
  pub funs: Vec<FunDef>,
  /// The variable definitions (imu/mut).
  pub vars: Vec<ExportedVar>,
  /// The enum definitions.
  pub enums: Vec<ExportedEnum>,
  /// The struct definitions.
  pub structs: Vec<ExportedStruct>,
  /// The compile-time constants (val).
  pub consts: Vec<ExportedConst>,
  /// The SIR instruction stream for codegen merging.
  pub sir_instructions: Vec<Insn>,
  /// The next value id (for ValueId offset).
  pub next_value_id: u32,
  /// The next label id (for Label / Jump / BranchIfNot
  /// offset when merging into the main SIR).
  pub next_label_id: u32,
  /// Paths re-exported via `pub load X::*;`. Consumers of
  /// this module fold each target's `exported` scope into
  /// their own scope. Plain `load X::*;` (private) does NOT
  /// land here.
  pub re_exports: Vec<Vec<Symbol>>,
  /// Symbols requested by a selective `load M::(foo, bar);`
  /// that DO exist in M's SIR but are NOT `pub`. Populated
  /// only when `selective` is set; consumers iterate this
  /// to emit `PrivateItemInLoad` at the load span.
  pub private_selective_hits: Vec<Symbol>,
  /// Generic apply-block bodies the importer must splice
  /// into its own tree to re-execute on mono dispatch. See
  /// [`ExportedGenericBody`].
  pub generic_bodies: Vec<ExportedGenericBody>,
  /// `(abstract_name, target_type) -> AbstractImpl` for
  /// every `apply Abstract for Type { ... }` block the
  /// module declared in a public pack. Importers fold
  /// these into their own dispatch table so `a == b` on
  /// a struct defined in a sibling module routes to the
  /// custom `Type::eq` instead of falling back to a
  /// primitive pointer compare.
  pub abstract_impls: FxHashMap<(Symbol, Symbol), AbstractImpl>,
}

/// Extracts pub exports from a compiled module's SIR.
///
/// Translates symbol names and TyIds from the module's
/// interner/type checker into the caller's.
///
/// If `selective` is `Some(name)`, only the matching export
/// is included.
pub fn extract_exports(
  sir: Sir,
  selective: Option<&str>,
  interner: &Interner,
  src_funs: &[zo_value::FunDef],
  src_generic_bodies: Vec<ExportedGenericBody>,
  src_abstract_impls: FxHashMap<(Symbol, Symbol), AbstractImpl>,
) -> ModuleExports {
  // Funs that ship a generic body — only these need
  // `type_params` carried across. Without this filter,
  // pre-existing apply-block type-param leakage (e.g.
  // `apply []int` methods inheriting the previous
  // `apply []$T`'s `$T`) sees `src_fun.type_params`
  // non-empty and tricks the importer's dot-call
  // dispatcher into the mono branch for a method that
  // actually has no generic to substitute.
  let generic_body_names: rustc_hash::FxHashSet<Symbol> =
    src_generic_bodies.iter().map(|b| b.name).collect();

  let mut funs = Vec::new();
  let mut vars = Vec::new();
  let mut enums = Vec::new();
  let mut structs = Vec::new();
  let mut consts = Vec::new();
  let mut re_exports = Vec::new();
  let mut private_selective_hits = Vec::new();
  let mut current_pack: Option<Symbol> = None;

  for insn in &sir.instructions {
    match insn {
      Insn::PackDecl { name, .. } => {
        current_pack = Some(*name);
      }
      // `pub load X::*;` records X as a re-export so
      // consumers of this module fold X's `exported`
      // scope into their own. Plain `load X::*;` is
      // private — it never reaches re_exports.
      Insn::ModuleLoad {
        path,
        pubness: Pubness::Yes,
        ..
      } => {
        re_exports.push(path.clone());
      }
      Insn::FunDef {
        name,
        params,
        return_ty,
        body_start,
        kind,
        pubness,
        self_kind,
        owning_pack,
        span,
        ..
      } => {
        let fn_name = interner.get(*name);

        if *pubness != Pubness::Yes {
          // A selective `load M::(foo);` over a non-pub `foo`
          // is a privacy violation, not a missing item. Record
          // the hit so the caller can emit `PrivateItemInLoad`
          // at the load span.
          if let Some(filter) = selective
            && fn_name == filter
          {
            private_selective_hits.push(*name);
          }
          continue;
        }

        if let Some(filter) = selective
          && !matches_selective(fn_name, filter)
        {
          continue;
        }

        // Shared interner: symbols are already in the same
        // namespace — no translation needed. TyIds still need
        // translation until TyChecker is shared.
        let dst_params =
          params.iter().map(|(p, ty)| (*p, *ty)).collect::<Vec<_>>();

        let dst_return_ty = *return_ty;

        // Carry return_type_args as-is — they're Ty values
        // (not TyIds) so they don't need translation.
        let src_fun = src_funs.iter().find(|f| f.name == *name);
        let rta = src_fun
          .map(|f| f.return_type_args.clone())
          .unwrap_or_default();

        // Carry `type_params` ONLY for array-apply funs that
        // ship a generic body (`arr_$::*`). Struct/enum
        // applies (`Vec`, `HashMap`, ...) rely on the
        // pre-existing dispatch path where the importer
        // synthesizes mono on demand from `local_struct_type_args`
        // at the call site — propagating `type_params` for
        // them would re-route through the per-call mono
        // branch and bypass the construction-time
        // substitution that already works. Array dispatch
        // has no such fallback, so `type_params` must ride
        // along to make `nums.first()` resolve `$T`.
        let name_str = interner.get(*name);
        let type_params = if generic_body_names.contains(name)
          && name_str.starts_with("arr_$::")
        {
          src_fun.map(|f| f.type_params.clone()).unwrap_or_default()
        } else {
          Vec::new()
        };

        let type_param_bounds = src_fun
          .map(|f| f.type_param_bounds.clone())
          .unwrap_or_default();

        funs.push(FunDef {
          name: *name,
          params: dst_params,
          return_ty: dst_return_ty,
          body_start: *body_start,
          kind: *kind,
          pubness: *pubness,
          type_params,
          type_param_bounds,
          return_type_args: rta,
          self_kind: *self_kind,
          owning_pack: *owning_pack,
          span: *span,
          is_test: false,
        });
      }

      Insn::VarDef {
        name,
        ty_id,
        init,
        pubness,
        ..
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let var_name = interner.get(*name);

        if let Some(filter) = selective
          && var_name != filter
        {
          continue;
        }

        let dst_ty_id = *ty_id;

        let literal = init.and_then(|vid| {
          sir.instructions.iter().find_map(|i| match i {
            Insn::ConstInt { dst, value, .. } if *dst == vid => {
              Some(ExportedLiteral::Int(*value))
            }
            Insn::ConstFloat { dst, value, .. } if *dst == vid => {
              Some(ExportedLiteral::Float(*value))
            }
            Insn::ConstBool { dst, value, .. } if *dst == vid => {
              Some(ExportedLiteral::Int(if *value { 1 } else { 0 }))
            }
            Insn::ConstString { dst, symbol, .. } if *dst == vid => {
              Some(ExportedLiteral::StringSym(*symbol))
            }
            _ => None,
          })
        });

        vars.push(ExportedVar {
          name: *name,
          ty_id: dst_ty_id,
          init: *init,
          owning_pack: current_pack,
          literal,
        });
      }

      Insn::EnumDef {
        name,
        variants,
        pubness,
        ..
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let enum_name = interner.get(*name);

        if let Some(filter) = selective
          && enum_name != filter
        {
          continue;
        }

        // Translate field types into the caller's type-checker.
        // The importing executor will re-intern the full enum
        // from this raw data so the EnumTyId lives in its
        // own table. Variant names are already shared via
        // the common interner.
        let dst_variants: Vec<(Symbol, u32, Vec<TyId>)> = variants
          .iter()
          .map(|(vname, disc, fields)| {
            let dst_fields: Vec<TyId> = fields.to_vec();

            (*vname, *disc, dst_fields)
          })
          .collect();

        enums.push(ExportedEnum {
          name: *name,
          variants: dst_variants,
        });
      }
      Insn::StructDef {
        name,
        ty_id,
        fields,
        pubness,
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let struct_name = interner.get(*name);

        if let Some(filter) = selective
          && struct_name != filter
        {
          continue;
        }

        structs.push(ExportedStruct {
          name: *name,
          ty_id: *ty_id,
          fields: fields.clone(),
        });
      }

      Insn::ConstDef {
        name,
        ty_id,
        value,
        pubness,
      } => {
        if *pubness != Pubness::Yes {
          continue;
        }

        let const_name = interner.get(*name);

        if let Some(filter) = selective
          && const_name != filter
        {
          continue;
        }

        consts.push(ExportedConst {
          name: *name,
          ty_id: *ty_id,
          value: *value,
        });

        // Also export as ExportedVar so the import
        // pipeline (which only reads `vars`) can
        // materialize the value in the consuming module.
        let literal = sir.instructions.iter().find_map(|i| match i {
          Insn::ConstInt { dst, value: v, .. } if *dst == *value => {
            Some(ExportedLiteral::Int(*v))
          }
          Insn::ConstBool { dst, value: v, .. } if *dst == *value => {
            Some(ExportedLiteral::Int(if *v { 1 } else { 0 }))
          }
          Insn::ConstString { dst, symbol, .. } if *dst == *value => {
            Some(ExportedLiteral::StringSym(*symbol))
          }
          _ => None,
        });

        vars.push(ExportedVar {
          name: *name,
          ty_id: *ty_id,
          init: Some(*value),
          owning_pack: current_pack,
          literal,
        });
      }

      _ => {}
    }
  }

  // Generic-outer-pass funs (`apply []$T { fun map<$U> }`
  // etc.) never emit an `Insn::FunDef` — the outer pass
  // skips the body and `reexecute_generic_instantiations`
  // replays the tree range per instantiation. The SIR-only
  // walk above therefore misses them; pull straight from
  // `src_funs` so importing modules see the signature and
  // route dispatch into the generic body.
  //
  // Gate on `arr_$::*` plus non-empty `type_params` so
  // struct/enum-applied generics (`Vec`, `HashMap`) keep
  // flowing through their existing construction-time
  // `local_struct_type_args` path — they don't need this
  // body-export channel.
  let already_exported: rustc_hash::FxHashSet<Symbol> =
    funs.iter().map(|f| f.name).collect();

  // `already_exported` must track BOTH the SIR-walk
  // pushes above AND the loop's own pushes — otherwise
  // duplicates within `src_funs` (an `arr_$::first` that
  // reached `mod_sem.funs` once via the cumulative
  // imports and again as the module's own outer-pass
  // entry) each push past the static snapshot. With
  // re-exports cascading the same symbols through
  // multiple paths, `src_funs` typically holds K copies
  // of each `arr_$::*` fun — without an in-loop update
  // every module's exports.funs balloons to ~src_funs
  // size and `fold_imports_into` doubles the import
  // graph at every depth.
  let mut already_exported = already_exported;

  for src_fun in src_funs {
    let name_str = interner.get(src_fun.name);

    if !name_str.starts_with("arr_$::") {
      continue;
    }

    if src_fun.type_params.is_empty() {
      continue;
    }

    if already_exported.contains(&src_fun.name) {
      continue;
    }

    if src_fun.pubness != zo_value::Pubness::Yes {
      continue;
    }

    if let Some(filter) = selective
      && !matches_selective(name_str, filter)
    {
      continue;
    }

    already_exported.insert(src_fun.name);

    funs.push(FunDef {
      name: src_fun.name,
      params: src_fun.params.clone(),
      return_ty: src_fun.return_ty,
      body_start: src_fun.body_start,
      kind: src_fun.kind,
      pubness: src_fun.pubness,
      type_params: src_fun.type_params.clone(),
      type_param_bounds: src_fun.type_param_bounds.clone(),
      return_type_args: src_fun.return_type_args.clone(),
      self_kind: src_fun.self_kind,
      owning_pack: src_fun.owning_pack,
      span: Span::ZERO,
      is_test: false,
    });
  }

  // Filter out EnumDef only — its ty_ids reference the
  // module's throwaway type checker, so leaving them in the
  // merged SIR causes ty_id collisions in the codegen's
  // enum_metas HashMap. `PackDecl` and `PackLink` MUST be
  // preserved: codegen's FFI pre-pass walks the merged SIR
  // tracking the most recent `PackDecl` to associate each
  // `pub ffi` declaration with its declaring pack, and uses
  // `PackLink` to resolve that pack's host dylib path.
  let sir_instructions = sir
    .instructions
    .into_iter()
    .filter(|i| !matches!(i, Insn::EnumDef { .. }))
    .collect();

  // Selective imports filter funs/vars/etc. above, but a
  // generic apply-block body has no public method name on
  // its own — it ships with whichever public fun pulled it
  // in. Pass the bodies through unconditionally; the
  // importer's mono pipeline only consults them when a
  // pending instantiation looks them up by mangled symbol.
  //
  // Abstract impls: travel with the type. A `pub apply Eq
  // for Point` from a public pack rides whenever Point is
  // visible to the importer. Privacy gate is per-entry
  // (`AbstractImpl.pubness`) so a private `pack foo;`
  // declaring an internal impl stays inside the pack —
  // mirrors the orphan-rule shape Rust enforces at the
  // crate boundary.
  let abstract_impls: FxHashMap<(Symbol, Symbol), AbstractImpl> =
    src_abstract_impls
      .into_iter()
      .filter(|(_, impl_)| impl_.pubness == Pubness::Yes)
      .filter(|((_abs, ty), _)| match selective {
        Some(filter) => interner.get(*ty) == filter,
        None => true,
      })
      .collect();

  // Debug-only invariant: a module's exported fun set
  // CANNOT exceed its analyzer's full fun table.
  // `src_funs` is the executor's complete `funs` vector
  // (own + every transitively-imported pub). `funs` is
  // what we ship to importers. If exports grow past that
  // bound, the snapshot-then-mutate family has struck
  // again — an `already_exported` set frozen before a
  // loop, a duplicate slipping through fold_imports_into,
  // an `arr_$::*` push that doesn't update its dedup
  // tracker. Catches the misato-class explosion at the
  // first test run instead of hours later in a perf
  // sweep.
  debug_assert!(
    funs.len() <= src_funs.len(),
    "extract_exports leaked duplicates: funs.len()={} src_funs.len()={}",
    funs.len(),
    src_funs.len(),
  );

  ModuleExports {
    funs,
    vars,
    enums,
    structs,
    consts,
    sir_instructions,
    next_value_id: sir.next_value_id,
    next_label_id: sir.next_label_id,
    re_exports,
    private_selective_hits,
    generic_bodies: src_generic_bodies,
    abstract_impls,
  }
}
