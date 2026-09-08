//! This module contains unit structs that should be used inside `quote!` and `spanned_quote!`
//! using the variable interpolation syntax in place of their equivalent structs and traits
//! present in `std`.
//!
//! To create hygienic proc macros, all the names must be its fully qualified form. These
//! unit structs help us to not specify the fully qualified name every single time.
//!
//! # Example
//! Instead of writing this:
//! ```
//! # use quote::quote;
//! quote!(
//!     fn get_id() -> Option<i32> {
//!         Some(0)
//!     }
//! );
//! ```
//! Or this:
//! ```
//! # use quote::quote;
//! quote!(
//!     fn get_id() -> ::core::option::Option<i32> {
//!         ::core::option::Option::Some(0)
//!     }
//! );
//! ```
//! We should write this:
//! ```
//! use bevy_macro_utils::fq_std::FQOption;
//! # use quote::quote;
//!
//! quote!(
//!     fn get_id() -> #FQOption<i32> {
//!         #FQOption::Some(0)
//!     }
//! );
//! ```

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

/// Fully Qualified (FQ) short name for [`std::any::Any`]
pub struct FQAny;
/// Fully Qualified (FQ) short name for [`Box`]
pub struct FQBox;
/// Fully Qualified (FQ) short name for [`Clone`]
pub struct FQClone;
/// Fully Qualified (FQ) short name for [`Default`]
pub struct FQDefault;
/// Fully Qualified (FQ) short name for [`Option`]
pub struct FQOption;
/// Fully Qualified (FQ) short name for [`Result`]
pub struct FQResult;
/// Fully Qualified (FQ) short name for [`Send`]
pub struct FQSend;
/// Fully Qualified (FQ) short name for [`Sync`]
pub struct FQSync;
/// Fully Qualified (FQ) short name for [`Into`]
pub struct FQInto;
/// Fully Qualified (FQ) short name for [`Iterator`]
pub struct FQIterator;

impl ToTokens for FQAny {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::any::Any).to_tokens(tokens);
    }
}

impl ToTokens for FQClone {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::clone::Clone).to_tokens(tokens);
    }
}

impl ToTokens for FQDefault {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::default::Default).to_tokens(tokens);
    }
}

impl ToTokens for FQOption {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::option::Option).to_tokens(tokens);
    }
}

impl ToTokens for FQResult {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::result::Result).to_tokens(tokens);
    }
}

impl ToTokens for FQSend {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::marker::Send).to_tokens(tokens);
    }
}

impl ToTokens for FQSync {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::marker::Sync).to_tokens(tokens);
    }
}

impl ToTokens for FQInto {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::convert::Into).to_tokens(tokens);
    }
}

impl ToTokens for FQIterator {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        quote!(::std::iter::Iterator).to_tokens(tokens);
    }
}
