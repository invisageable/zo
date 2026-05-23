use zo_codegen_arm::ARM64Gen;
use zo_codegen_backend::{Artifact, Backend, LinkObject, Target};
use zo_codegen_clif::CliftGen;
use zo_interner::{Interner, Symbol};
use zo_module_resolver::{AbstractDef, AbstractImpl};
use zo_sir::Sir;
use zo_ty::{Ty, TyTable};

use rustc_hash::FxHashMap;

/// Optional abstract-state slice plumbed through codegen
/// for the dynamic-dispatch (`any <Abstract>`) pipeline.
/// `defs` enumerates each abstract's methods (the order
/// keys the vtable slot index); `impls` records the
/// concrete-type bindings whose vtables the codegen
/// emits. Threaded only into the ARM backend at present
/// — Cranelift ignores it.
pub struct AbstractState {
  pub defs: FxHashMap<Symbol, AbstractDef>,
  pub impls: FxHashMap<(Symbol, Symbol), AbstractImpl>,
}

/// Concrete backend selected per [`Target`]. The common
/// `generate` path routes through the [`Backend`] trait;
/// ARM-specific post-processing (Mach-O writer, executable
/// bit, asm text) stays on the `ARM64Gen` arm. `ARM64Gen`
/// is ~700 bytes so it's boxed to keep the enum compact —
/// `Concrete` is held briefly on the stack in `make_backend`.
enum Concrete<'a> {
  Arm64(Box<ARM64Gen<'a>>),
  Clift(CliftGen<'a>),
}

/// Represents the [`Codegen`] dispatcher.
pub struct Codegen {
  target: Target,
}

impl Codegen {
  /// Creates a new [`Codegen`] instance.
  pub const fn new(target: Target) -> Self {
    Self { target }
  }

  /// Instantiates the backend matching `self.target`. The
  /// optional `(tys, ty_table)` view enables ARM64Gen's
  /// generic AAPCS FFI fallback — when `Some`, calls to a
  /// `FunctionKind::Intrinsic` symbol that no per-symbol
  /// arm matched are routed through `abi::classify` +
  /// `emit_ffi_call`. CLIF ignores the view (its FFI path
  /// uses Cranelift's own ABI lowering).
  fn make_backend<'a>(
    &self,
    interner: &'a Interner,
    type_view: Option<(&'a [Ty], &'a TyTable)>,
    abstract_state: Option<AbstractState>,
  ) -> Concrete<'a> {
    match self.target {
      Target::Arm64AppleDarwin | Target::Arm64UnknownLinuxGnu => {
        let mut arm = ARM64Gen::new(interner);
        if let Some((tys, ty_table)) = type_view {
          arm = arm.with_type_view(tys, ty_table);
        }
        if let Some(state) = abstract_state {
          arm = arm.with_abstract_state(state.defs, state.impls);
        }
        Concrete::Arm64(Box::new(arm))
      }
      Target::X8664AppleDarwin
      | Target::X8664UnknownLinuxGnu
      | Target::X8664PcWindowsMsvc
      | Target::Arm64PcWindowsMsvc => {
        Concrete::Clift(CliftGen::new(interner, self.target))
      }
      Target::Wasm32UnknownUnknown => todo!("wasm backend not yet wired"),
    }
  }

  /// Run the codegen phase — turn `sir` into a
  /// [`LinkObject`] ready for the linker.
  ///
  /// No I/O happens here; writing the executable is the
  /// linker's job (`zo-linker::link`). The returned
  /// `LinkObject` is the only data that crosses the
  /// codegen → linker phase boundary — every fixup,
  /// symbol table, and entry-point offset the linker
  /// needs is materialized into it.
  ///
  /// ARM produces `LinkObject::Macho` (raw machine code +
  /// symbol/fixup tables for in-process mach-o assembly).
  /// CLIF produces `LinkObject::Object` (a relocatable
  /// object file ready for `cc`).
  pub fn generate(
    self,
    interner: &Interner,
    sir: &Sir,
    type_view: Option<(&[Ty], &TyTable)>,
    abstract_state: Option<AbstractState>,
  ) -> LinkObject {
    match self.make_backend(interner, type_view, abstract_state) {
      Concrete::Arm64(mut codegen) => {
        let artifact = codegen.generate(sir);

        LinkObject::Macho(Box::new(codegen.into_link_object(artifact)))
      }
      Concrete::Clift(mut codegen) => {
        let artifact = codegen.generate(sir);

        LinkObject::Object(artifact.code)
      }
    }
  }

  /// Generates the [`Artifact`].
  pub fn generate_artifact(
    &self,
    interner: &Interner,
    sir: &Sir,
    type_view: Option<(&[Ty], &TyTable)>,
    abstract_state: Option<AbstractState>,
  ) -> Artifact {
    match self.make_backend(interner, type_view, abstract_state) {
      Concrete::Arm64(mut codegen) => codegen.generate(sir),
      Concrete::Clift(mut codegen) => codegen.generate(sir),
    }
  }

  /// Generates assembly text for display. ARM returns
  /// disassembled ARM64; Cranelift returns CLIF IR text (pre-
  /// machine-code — equivalently useful for debugging the
  /// backend's own decisions, and avoids pulling a disassembler
  /// dep into the CLIF path).
  pub fn generate_asm(
    &self,
    interner: &Interner,
    sir: &Sir,
    type_view: Option<(&[Ty], &TyTable)>,
    abstract_state: Option<AbstractState>,
  ) -> String {
    match self.make_backend(interner, type_view, abstract_state) {
      Concrete::Arm64(mut codegen) => codegen.generate_asm(sir),
      Concrete::Clift(mut codegen) => codegen.generate_asm(sir),
    }
  }
}
