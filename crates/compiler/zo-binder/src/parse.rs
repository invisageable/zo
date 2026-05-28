//! Parse a Rust shim's source into [`FfiItem`]s.
//!
//! @note — keeps only `extern "C"` functions carrying
//! `#[no_mangle]` or `#[unsafe(no_mangle)]`; everything else
//! is skipped.

use crate::model::{BindError, FfiItem, Param, RustTy};

use quote::ToTokens;
use syn::{
  Attribute, Expr, ExprLit, FnArg, Item, ItemFn, Lit, Meta, Pat, ReturnType,
  Type,
};

/// Extract every C-ABI, `no_mangle` function from `src`.
pub fn parse_ffi_items(src: &str) -> Result<Vec<FfiItem>, BindError> {
  let file =
    syn::parse_file(src).map_err(|err| BindError::Syntax(err.to_string()))?;

  let items = file
    .items
    .iter()
    .filter_map(|item| match item {
      Item::Fn(fun) if is_ffi_fn(fun) => Some(ffi_item(fun)),
      _ => None,
    })
    .collect();

  Ok(items)
}

/// True when `fun` is `extern "C"` and `no_mangle`.
fn is_ffi_fn(fun: &ItemFn) -> bool {
  is_extern_c(fun) && has_no_mangle(&fun.attrs)
}

/// True when the function's ABI is `extern "C"`.
fn is_extern_c(fun: &ItemFn) -> bool {
  match &fun.sig.abi {
    // bare `extern` defaults to the C ABI.
    Some(abi) => abi.name.as_ref().is_none_or(|n| n.value() == "C"),
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

/// Build an [`FfiItem`] from a validated function.
fn ffi_item(fun: &ItemFn) -> FfiItem {
  FfiItem {
    name: fun.sig.ident.to_string(),
    params: params(fun),
    ret: ret_ty(&fun.sig.output),
    doc: doc_lines(&fun.attrs),
  }
}

/// Collect the typed parameters, skipping any `self`.
fn params(fun: &ItemFn) -> Vec<Param> {
  fun
    .sig
    .inputs
    .iter()
    .filter_map(|arg| match arg {
      FnArg::Typed(pat) => Some(Param {
        name: pat_name(&pat.pat),
        ty: rust_ty(&pat.ty),
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
