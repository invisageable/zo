//! Parse a Rust shim's source into [`FfiItem`]s.
//!
//! @note — keeps only `extern "C"` functions carrying
//! `#[no_mangle]` or `#[unsafe(no_mangle)]`; everything else
//! is skipped. `type X = Y;` aliases are resolved so the type
//! mapper sees concrete types.

use crate::model::{BindError, FfiItem, Param, RustTy};

use quote::ToTokens;
use syn::{
  Attribute, Expr, ExprLit, File, FnArg, Item, ItemFn, Lit, Meta, Pat,
  ReturnType, Type,
};

use std::collections::HashMap;

/// Bound on alias resolution depth — guards cyclic `type`
/// definitions without panicking.
const MAX_ALIAS_DEPTH: usize = 16;

/// Extract every C-ABI, `no_mangle` function from `src`.
pub fn parse_ffi_items(src: &str) -> Result<Vec<FfiItem>, BindError> {
  let file =
    syn::parse_file(src).map_err(|err| BindError::Syntax(err.to_string()))?;

  let aliases = collect_aliases(&file);

  let items = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Fn(fun) if is_ffi_fn(fun) => Some(ffi_item(fun, &aliases)),
      _ => None,
    })
    .collect();

  Ok(items)
}

/// Collect `type X = Y;` aliases into a name → type map.
fn collect_aliases(file: &File) -> HashMap<String, RustTy> {
  file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Type(alias) => Some((alias.ident.to_string(), rust_ty(&alias.ty))),
      _ => None,
    })
    .collect()
}

/// True when `fun` is `extern "C"` and `no_mangle`.
fn is_ffi_fn(fun: &ItemFn) -> bool {
  is_extern_c(fun) && has_no_mangle(&fun.attrs)
}

/// True when the function's ABI is `extern "C"`.
fn is_extern_c(fun: &ItemFn) -> bool {
  match &fun.sig.abi {
    // bare `extern` defaults to the C ABI.
    Some(abi) => abi.name.as_ref().is_none_or(|name| name.value() == "C"),
    None => false,
  }
}

/// True when the attrs carry `#[no_mangle]` or
/// `#[unsafe(no_mangle)]`.
fn has_no_mangle(attrs: &[Attribute]) -> bool {
  attrs.iter().any(|attr| {
    if attr.path().is_ident("no_mangle") {
      return true;
    }

    // 2024 unsafe-attribute form: `#[unsafe(no_mangle)]`.
    if attr.path().is_ident("unsafe")
      && let Meta::List(list) = &attr.meta
    {
      return list.tokens.to_string().contains("no_mangle");
    }

    false
  })
}

/// Build an [`FfiItem`], resolving aliases in its types.
fn ffi_item(fun: &ItemFn, aliases: &HashMap<String, RustTy>) -> FfiItem {
  FfiItem {
    name: fun.sig.ident.to_string(),
    params: params(fun, aliases),
    ret: resolve(&ret_ty(&fun.sig.output), aliases),
    doc: doc_lines(&fun.attrs),
  }
}

/// Collect the typed parameters, skipping any `self`.
fn params(fun: &ItemFn, aliases: &HashMap<String, RustTy>) -> Vec<Param> {
  fun
    .sig
    .inputs
    .iter()
    .filter_map(|arg| match arg {
      FnArg::Typed(pat) => Some(Param {
        name: pat_name(&pat.pat),
        ty: resolve(&rust_ty(&pat.ty), aliases),
      }),
      FnArg::Receiver(_) => None,
    })
    .collect()
}

/// The binding name of a parameter pattern.
fn pat_name(pat: &Pat) -> String {
  match pat {
    Pat::Ident(ident) => ident.ident.to_string(),
    other => other.to_token_stream().to_string(),
  }
}

/// Normalize a return type, mapping `-> ()` and the default
/// to [`RustTy::Unit`].
fn ret_ty(output: &ReturnType) -> RustTy {
  match output {
    ReturnType::Default => RustTy::Unit,
    ReturnType::Type(_, ty) => rust_ty(ty),
  }
}

/// Normalize a Rust type into the binder's [`RustTy`].
fn rust_ty(ty: &Type) -> RustTy {
  match ty {
    Type::Path(path) => match path.path.segments.last() {
      Some(seg) => RustTy::Path(seg.ident.to_string()),
      None => RustTy::Other(ty.to_token_stream().to_string()),
    },
    Type::Ptr(ptr) => RustTy::Ptr {
      mutable: ptr.mutability.is_some(),
      inner: Box::new(rust_ty(&ptr.elem)),
    },
    Type::Tuple(tuple) if tuple.elems.is_empty() => RustTy::Unit,
    other => RustTy::Other(other.to_token_stream().to_string()),
  }
}

/// Resolve `type` aliases in `ty` to their concrete types.
fn resolve(ty: &RustTy, aliases: &HashMap<String, RustTy>) -> RustTy {
  resolve_guarded(ty, aliases, 0)
}

/// Resolve aliases with a depth guard against cycles.
fn resolve_guarded(
  ty: &RustTy,
  aliases: &HashMap<String, RustTy>,
  depth: usize,
) -> RustTy {
  if depth > MAX_ALIAS_DEPTH {
    return ty.clone();
  }

  match ty {
    RustTy::Path(name) => match aliases.get(name) {
      Some(target) => resolve_guarded(target, aliases, depth + 1),
      None => ty.clone(),
    },
    RustTy::Ptr { mutable, inner } => RustTy::Ptr {
      mutable: *mutable,
      inner: Box::new(resolve_guarded(inner, aliases, depth + 1)),
    },
    other => other.clone(),
  }
}

/// Collect `///` doc lines (lowered to `#[doc = "..."]`).
fn doc_lines(attrs: &[Attribute]) -> Vec<String> {
  attrs
    .iter()
    .filter_map(|attr| {
      if !attr.path().is_ident("doc") {
        return None;
      }

      match &attr.meta {
        Meta::NameValue(nv) => match &nv.value {
          Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
          }) => Some(s.value().trim().to_string()),
          _ => None,
        },
        _ => None,
      }
    })
    .collect()
}
