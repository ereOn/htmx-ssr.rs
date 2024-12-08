//! HTMX-SSR macros

use syn::parse_macro_input;

mod display_delegate;
mod fragment;
mod route;

/// Create an HTMX fragment.
///
/// This derive macro simply implements the `Fragment` trait for the annotated type.
#[proc_macro_derive(Fragment, attributes(htmx))]
pub fn derive_fragment(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(input as syn::DeriveInput);

    fragment::derive(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive a route type.
///
/// Route types are enum types that represent the possible routes in an HTMX application.
#[proc_macro_derive(Route, attributes(route, subroute, query, body))]
pub fn derive_route(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(input as syn::DeriveInput);

    route::derive(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implement the `Display` trait for an enum.
///
/// This derive macro simply implements the `Display` trait for the annotated enum type, by
/// delegating it to the `Display` implementation of the inner variants.
#[proc_macro_derive(DisplayDelegate)]
pub fn derive_display_delegate(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut input = parse_macro_input!(input as syn::DeriveInput);

    display_delegate::derive(&mut input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
