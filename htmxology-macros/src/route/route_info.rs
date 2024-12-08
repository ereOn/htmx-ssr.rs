//! A route method.

use proc_macro2::{Span, TokenStream};
use quote::{quote, ToTokens};
use syn::Ident;

use super::RouteUrl;

/// A route info.
///
/// This is a subset of the standard HTTP methods, that are commonly used in web applications.
#[derive(Debug, Clone)]
pub enum RouteInfo {
    /// A simple HTTP route.
    Simple {
        /// The route URL.
        url: RouteUrl,

        /// The route method.
        method: String,
    },

    /// A prefixed sub-route.
    SubRoute {
        /// The prefix.
        prefix: RouteUrl,
    },
}

impl RouteInfo {
    /// Get the URL as a string, failing if there are any required path parameters.
    pub fn to_unparameterized_string(
        &self,
        ctx: impl ToTokens,
        query_arg: Option<&Ident>,
    ) -> syn::Result<TokenStream> {
        let mut statements = match self {
            Self::Simple { url, .. } => url,
            Self::SubRoute { prefix } => prefix,
        }
        .to_unparameterized_string(ctx)?;

        Self::append_query_arg(&mut statements, query_arg);

        Ok(quote! { {
            #( #statements )*
        } })
    }

    /// Get the URL as format string, failing if there are any required path parameters.
    pub fn to_named_parameters_format(
        &self,
        t: &impl ToTokens,
        name_params: impl IntoIterator<Item = (String, Ident)>,
        query_arg: Option<&Ident>,
    ) -> syn::Result<TokenStream> {
        let mut statements = match self {
            Self::Simple { url, .. } => url,
            Self::SubRoute { prefix } => prefix,
        }
        .to_named_parameters_format(t, name_params)?;

        Self::append_query_arg(&mut statements, query_arg);

        Ok(quote! { {
            #( #statements )*
        } })
    }

    /// Get the URL as format string, failing if there are any required path parameters.
    pub fn to_unnamed_parameters_format(
        &self,
        ctx: &impl ToTokens,
        unnamed_params: impl IntoIterator<Item = Ident>,
        query_arg: Option<&Ident>,
    ) -> syn::Result<TokenStream> {
        let mut statements = match self {
            Self::Simple { url, .. } => url,
            Self::SubRoute { prefix } => prefix,
        }
        .to_unnamed_parameters_format(ctx, unnamed_params)?;

        Self::append_query_arg(&mut statements, query_arg);

        Ok(quote! { {
            #( #statements )*
        } })
    }

    fn append_query_arg(statements: &mut Vec<TokenStream>, query_arg: Option<&Ident>) {
        if let Some(query_arg) = query_arg {
            statements.push(quote! {
                std::fmt::Write::write_char(f, '?')?;
                let qs = &serde_urlencoded::to_string(&#query_arg).map_err(|_| std::fmt::Error)?;
                f.write_str(&qs)?;
            });
        }
    }

    /// Get the Axum router registration.
    pub fn to_axum_route_registration(&self, handler: TokenStream) -> TokenStream {
        match self {
            Self::Simple { url, method } => {
                let url = url.to_axum_router_path();
                let method = Ident::new(&method.to_ascii_lowercase(), Span::call_site());

                quote! {
                    .route(#url, axum::routing::#method(#handler))
                }
            }
            Self::SubRoute { prefix } => {
                let prefix = prefix.to_axum_router_path();

                quote! {
                    .nest(#prefix, #handler)
                }
            }
        }
    }
}
