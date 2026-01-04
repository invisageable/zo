//! Derive macros for the eazy animation library.
//!
//! Provides `#[derive(Tweenable)]` to automatically implement the
//! `Tweenable` trait for custom structs.
//!
//! # Examples
//!
//! ```rust,ignore
//! use eazy_tweener::Tweenable;
//!
//! #[derive(Clone, Copy, Tweenable)]
//! struct Position {
//!   x: f32,
//!   y: f32,
//!   z: f32,
//! }
//!
//! let a = Position { x: 0.0, y: 0.0, z: 0.0 };
//! let b = Position { x: 100.0, y: 200.0, z: 300.0 };
//! let mid = a.lerp(b, 0.5);
//!
//! assert_eq!(mid.x, 50.0);
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, DeriveInput, Fields, parse_macro_input};

/// Derive the `Tweenable` trait for a struct.
///
/// All fields must implement `Tweenable`. The struct must also implement
/// `Copy`, `Send`, `Sync`, and have a `'static` lifetime.
///
/// # Examples
///
/// ## Named fields
///
/// ```rust,ignore
/// #[derive(Clone, Copy, Tweenable)]
/// struct Color {
///   r: f32,
///   g: f32,
///   b: f32,
///   a: f32,
/// }
/// ```
///
/// ## Tuple struct
///
/// ```rust,ignore
/// #[derive(Clone, Copy, Tweenable)]
/// struct Vec2(f32, f32);
/// ```
#[proc_macro_derive(Tweenable)]
pub fn derive_tweenable(input: TokenStream) -> TokenStream {
  let input = parse_macro_input!(input as DeriveInput);
  let struct_name = &input.ident;
  let generics = &input.generics;
  let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

  let expanded = match &input.data {
    Data::Struct(data_struct) => match &data_struct.fields {
      // Named fields: struct Foo { x: f32, y: f32 }
      Fields::Named(fields) => {
        let field_lerps = fields.named.iter().map(|field| {
          let name = &field.ident;

          quote! {
            #name: self.#name.lerp(other.#name, t)
          }
        });

        quote! {
          impl #impl_generics eazy::Tweenable for #struct_name #ty_generics #where_clause {
            #[inline(always)]
            fn lerp(self, other: Self, t: f32) -> Self {
              Self {
                #(#field_lerps),*
              }
            }
          }
        }
      }

      // Tuple struct: struct Foo(f32, f32)
      Fields::Unnamed(fields) => {
        let field_lerps = fields.unnamed.iter().enumerate().map(|(i, _)| {
          let index = syn::Index::from(i);

          quote! {
            self.#index.lerp(other.#index, t)
          }
        });

        quote! {
          impl #impl_generics eazy::Tweenable for #struct_name #ty_generics #where_clause {
            #[inline(always)]
            fn lerp(self, other: Self, t: f32) -> Self {
              Self(
                #(#field_lerps),*
              )
            }
          }
        }
      }

      // Unit struct: struct Foo;
      Fields::Unit => {
        quote! {
          impl #impl_generics eazy::Tweenable for #struct_name #ty_generics #where_clause {
            #[inline(always)]
            fn lerp(self, _other: Self, _t: f32) -> Self {
              self
            }
          }
        }
      }
    },

    Data::Enum(_) => {
      return syn::Error::new_spanned(
        input,
        "Tweenable cannot be derived for enums",
      )
      .to_compile_error()
      .into();
    }

    Data::Union(_) => {
      return syn::Error::new_spanned(
        input,
        "Tweenable cannot be derived for unions",
      )
      .to_compile_error()
      .into();
    }
  };

  TokenStream::from(expanded)
}
