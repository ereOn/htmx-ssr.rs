//! Route derive macro.

use std::collections::BTreeMap;

use quote::{quote, quote_spanned};
use syn::{
    punctuated::Punctuated, spanned::Spanned, Data, Error, Expr, Fields, Ident, Token, Variant,
};

mod route_info;
mod route_url;

use route_info::RouteInfo;
use route_url::{ParseError, RouteUrl};

mod attributes {
    pub(super) const ROUTE: &str = "route";
    pub(super) const METHOD: &str = "method";
    pub(super) const SUBROUTE: &str = "subroute";
    pub(super) const QUERY: &str = "query";
    pub(super) const BODY: &str = "body";
}

pub(super) fn derive(input: &mut syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let root_ident = &input.ident;

    let data = match &input.data {
        Data::Struct(_) => {
            return Err(Error::new_spanned(
                root_ident,
                "can't derive Route for a struct",
            ));
        }
        Data::Enum(data_enum) => data_enum,
        Data::Union(_) => {
            return Err(Error::new_spanned(
                root_ident,
                "can't derive Route for a union",
            ));
        }
    };

    let mut routes = Vec::with_capacity(data.variants.len());
    let mut to_urls = Vec::with_capacity(data.variants.len());
    let mut declarations = Vec::with_capacity(data.variants.len());

    for variant in &data.variants {
        let ident = &variant.ident;

        let route_info = parse_route_info(variant)?;

        let handler = match &variant.fields {
            // Enum::Unit - no query or body parameters.
            Fields::Unit => {
                let url = route_info.to_unparameterized_string(variant, None)?;

                to_urls.push(quote! {
                    Self::#ident => #url
                });

                quote_spanned! { variant.span() =>
                    |
                        axum::extract::State(state): axum::extract::State<htmxology::State<_>>,
                        htmx: htmxology::htmx::Request,
                    | async move {
                        Controller::render_view(#root_ident::#ident, state, htmx).await
                    }
                }
            }
            // Enum::Named{} - no query or body parameters.
            Fields::Named(fields) if fields.named.is_empty() => {
                let url = route_info.to_unparameterized_string(variant, None)?;

                to_urls.push(quote! {
                    Self::#ident{} => #url
                });

                quote_spanned! { variant.span() =>
                    |
                        axum::extract::State(state): axum::extract::State<htmxology::State<_>>,
                        htmx: htmxology::htmx::Request,
                    | async move {
                        Controller::render_view(#root_ident::#ident{}, state, htmx).await
                    }
                }
            }
            // Enum::Unnamed() - no query or body parameters.
            Fields::Unnamed(fields) if fields.unnamed.is_empty() => {
                let url = route_info.to_unparameterized_string(variant, None)?;

                to_urls.push(quote! {
                    Self::#ident() => #url
                });

                quote_spanned! { variant.span() =>
                    |
                        axum::extract::State(state): axum::extract::State<htmxology::State<_>>,
                        htmx: htmxology::htmx::Request,
                    | async move {
                        Controller::render_view(#root_ident::#ident(), state, htmx).await
                    }
                }
            }
            // Enum::Named{...}
            Fields::Named(fields) => {
                // Will contain all the arguments.
                let mut args = Vec::with_capacity(fields.named.len());
                let mut args_defs = Vec::with_capacity(fields.named.len());

                // All the path arguments.
                let mut path_args = Vec::with_capacity(fields.named.len());
                let mut path_args_names = BTreeMap::new();

                // If there is a query argument, this will be set to its ident.
                let mut query_arg = None;

                // If there is a body argument, this will be set to its ident.
                let mut body_arg = None;

                for field in &fields.named {
                    let field_ident = field
                        .ident
                        .as_ref()
                        .expect("field of named variant has no ident");
                    let field_ty = &field.ty;

                    args.push(quote_spanned! { field_ident.span() =>
                        #field_ident
                    });
                    args_defs.push(quote_spanned! { field_ident.span() =>
                        #field_ident: #field_ty
                    });

                    let is_query = field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident(attributes::QUERY));

                    let is_body = field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident(attributes::BODY));

                    match (is_query, is_body) {
                        (true, true) => {
                            return Err(Error::new_spanned(
                                field,
                                "field cannot be both query and body parameter",
                            ));
                        }
                        (true, false) => {
                            if query_arg.is_some() {
                                return Err(Error::new_spanned(
                                    field,
                                    "only one field can be a query parameter",
                                ));
                            }

                            query_arg = Some(field_ident.clone());
                        }
                        (false, true) => {
                            if body_arg.is_some() {
                                return Err(Error::new_spanned(
                                    field,
                                    "only one field can be a body parameter",
                                ));
                            }

                            body_arg = Some(field_ident.clone());
                        }
                        (false, false) => {
                            path_args.push(quote_spanned! { field_ident.span() =>
                                #field_ident
                            });
                            path_args_names.insert(field_ident.to_string(), field_ident.clone());
                        }
                    };
                }

                let url = if path_args_names.is_empty() {
                    route_info.to_unparameterized_string(variant, query_arg.as_ref())?
                } else {
                    route_info.to_named_parameters_format(
                        variant,
                        path_args_names,
                        query_arg.as_ref(),
                    )?
                };

                to_urls.push(quote! {
                    Self::#ident{#(#args),*} => #url
                });

                let params = Ident::new(&format!("{root_ident}{ident}Params"), ident.span());

                let path_parse = if path_args.is_empty() {
                    quote!()
                } else {
                    quote! {
                        let axum::extract::Path((#(#path_args),*)) = axum::extract::Path::from_request_parts(&mut parts, state)
                            .await
                            .map_err(|err| err.into_response())?;
                    }
                };

                let query_parse = match query_arg {
                    None => quote!(),
                    Some(query_ident) => {
                        quote! {
                            let axum::extract::Query(#query_ident) =
                            axum::extract::Query::from_request_parts(&mut parts, state)
                                .await
                                .map_err(|err| err.into_response())?;
                        }
                    }
                };

                let body_parse = match body_arg {
                    None => quote!(),
                    Some(body_ident) => {
                        quote! {
                            let req = http::Request::from_parts(parts, body);
                            let axum::extract::Form(#body_ident) =
                            axum::extract::Form::from_request(req, state)
                                .await
                                .map_err(|err| err.into_response())?;
                        }
                    }
                };

                declarations.push(quote! {
                    struct #params {
                        #(#args_defs),*
                    }

                    #[axum::async_trait]
                    impl<S: Send + Sync> axum::extract::FromRequest<S> for #params {
                        type Rejection = axum::response::Response;

                        async fn from_request(
                            req: axum::extract::Request,
                            state: &S,
                        ) -> Result<Self, Self::Rejection> {
                            use axum::extract::FromRequestParts;

                            let (mut parts, body) = req.into_parts();
                            #path_parse
                            #query_parse
                            #body_parse

                            Ok(Self { #(#args),* })
                        }
                    }

                    impl From<#params> for #root_ident {
                        fn from(#params{ #(#args),* }: #params) -> Self {
                            Self::#ident { #(#args),* }
                        }
                    }
                });

                quote_spanned! { variant.span() =>
                    |
                        axum::extract::State(state): axum::extract::State<htmxology::State<_>>,
                        htmx: htmxology::htmx::Request,
                        params: #params,
                    | async move {
                        Controller::render_view(#root_ident::from(params), state, htmx).await
                    }
                }
            }
            // Enum::Unnamed(...)
            Fields::Unnamed(fields) => {
                // Will contain all the arguments.
                let mut args = Vec::with_capacity(fields.unnamed.len());
                let mut args_defs = Vec::with_capacity(fields.unnamed.len());

                // All the path arguments.
                let mut path_args = Vec::with_capacity(fields.unnamed.len());
                let mut path_args_unnamed = Vec::with_capacity(fields.unnamed.len());

                // If there is a query argument, this will be set to its ident.
                let mut query_arg = None;

                // If there is a body argument, this will be set to its ident.
                let mut body_arg = None;

                for (i, field) in fields.unnamed.iter().enumerate() {
                    let field_ident = Ident::new(&format!("arg{i}"), field.span());
                    let field_ty = &field.ty;

                    args.push(quote_spanned! { field_ident.span() =>
                        #field_ident
                    });
                    args_defs.push(quote_spanned! { field_ident.span() =>
                        #field_ident: #field_ty
                    });

                    let is_query = field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident(attributes::QUERY));

                    let is_body = field
                        .attrs
                        .iter()
                        .any(|attr| attr.path().is_ident(attributes::BODY));

                    match (is_query, is_body) {
                        (true, true) => {
                            return Err(Error::new_spanned(
                                field,
                                "field cannot be both query and body parameter",
                            ));
                        }
                        (true, false) => {
                            if query_arg.is_some() {
                                return Err(Error::new_spanned(
                                    field,
                                    "only one field can be a query parameter",
                                ));
                            }

                            query_arg = Some(field_ident.clone());
                        }
                        (false, true) => {
                            if body_arg.is_some() {
                                return Err(Error::new_spanned(
                                    field,
                                    "only one field can be a body parameter",
                                ));
                            }

                            body_arg = Some(field_ident.clone());
                        }
                        (false, false) => {
                            path_args.push(quote_spanned! { field_ident.span() =>
                                #field_ident
                            });
                            path_args_unnamed.push(field_ident.clone());
                        }
                    };
                }

                let url = if path_args.is_empty() {
                    route_info.to_unparameterized_string(variant, query_arg.as_ref())?
                } else {
                    route_info.to_unnamed_parameters_format(
                        variant,
                        path_args_unnamed,
                        query_arg.as_ref(),
                    )?
                };

                to_urls.push(quote! {
                    Self::#ident{#(#args),*} => #url
                });

                let params = Ident::new(&format!("{root_ident}{ident}Params"), ident.span());

                let path_parse = if path_args.is_empty() {
                    quote!()
                } else {
                    quote! {
                        let axum::extract::Path((#(#path_args),*)) = axum::extract::Path::from_request_parts(&mut parts, state)
                            .await
                            .map_err(|err| err.into_response())?;
                    }
                };

                let query_parse = match query_arg {
                    None => quote!(),
                    Some(query_ident) => {
                        quote! {
                            let axum::extract::Query(#query_ident) =
                            axum::extract::Query::from_request_parts(&mut parts, state)
                                .await
                                .map_err(|err| err.into_response())?;
                        }
                    }
                };

                let body_parse = match body_arg {
                    None => quote!(),
                    Some(body_ident) => {
                        quote! {
                            let req = http::Request::from_parts(parts, body);
                            let axum::extract::Form(#body_ident) =
                            axum::extract::Form::from_request(req, state)
                                .await
                                .map_err(|err| err.into_response())?;
                        }
                    }
                };

                declarations.push(quote! {
                    struct #params {
                        #(#args_defs),*
                    }

                    #[axum::async_trait]
                    impl<S: Send + Sync> axum::extract::FromRequest<S> for #params {
                        type Rejection = axum::response::Response;

                        async fn from_request(
                            req: req: axum::extract::Request,
                            state: &S,
                        ) -> Result<Self, Self::Rejection> {
                            use axum::extract::FromRequestParts;

                            let (mut parts, body) = req.into_parts();
                            #path_parse
                            #query_parse
                            #body_parse

                            Ok(Self { #(#args),* })
                        }
                    }

                    impl From<#params> for #root_ident {
                        fn from(#params{ #(#args),* }: #params) -> Self {
                            Self::#ident ( #(#args),* )
                        }
                    }
                });

                quote_spanned! { variant.span() =>
                    |
                        axum::extract::State(state): axum::extract::State<htmxology::State<_>>,
                        htmx: htmxology::htmx::Request,
                        params: #params,
                    | async move{
                        Controller::render_view(#root_ident::from(params), state, htmx).await
                    }
                }
            }
        };

        routes.push(route_info.to_axum_route_registration(handler));
    }

    Ok(quote! {
        #(#declarations)*

        impl htmxology::Route for #root_ident {
            fn make_router<Controller: htmxology::Controller<Route=Self>>() -> axum::Router<htmxology::State<Controller::Model>> {
                let router = axum::Router::new();

                router
                    #(#routes)*
            }
        }

        impl std::fmt::Display for #root_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    #(#to_urls),*
                };

                Ok(())
            }
        }
    })
}

fn parse_route_info(variant: &Variant) -> syn::Result<RouteInfo> {
    let mut result = None;

    for attr in &variant.attrs {
        if attr.path().is_ident(attributes::ROUTE) {
            if result.is_some() {
                return Err(Error::new_spanned(
                    attr,
                    format!(
                        "expected at most one `{}` or `{}` attribute",
                        attributes::ROUTE,
                        attributes::SUBROUTE
                    ),
                ));
            }

            let mut exprs = attr
                .parse_args_with(Punctuated::<syn::Expr, Token![,]>::parse_terminated)?
                .into_iter();

            let raw_url = exprs.next().ok_or_else(|| {
                Error::new_spanned(attr, "expected a route URL as the first argument")
            })?;

            let url = parse_route_url(raw_url)?;

            let method = match exprs.next() {
                Some(raw_method) => parse_method(raw_method)?,
                None => "GET".to_string(),
            };

            if exprs.next().is_none() {
                result = Some(RouteInfo::Simple { url, method });
            } else {
                return Err(Error::new_spanned(attr, "expected at most two arguments"));
            }
        } else if attr.path().is_ident(attributes::SUBROUTE) {
            if result.is_some() {
                return Err(Error::new_spanned(
                    attr,
                    format!(
                        "expected at most one `{}` or `{}` attribute",
                        attributes::ROUTE,
                        attributes::SUBROUTE
                    ),
                ));
            }

            let mut exprs = attr
                .parse_args_with(Punctuated::<syn::Expr, Token![,]>::parse_terminated)?
                .into_iter();

            let raw_url = exprs.next().ok_or_else(|| {
                Error::new_spanned(attr, "expected a route URL as the first argument")
            })?;

            let prefix = parse_route_url(raw_url)?;

            if exprs.next().is_none() {
                result = Some(RouteInfo::SubRoute { prefix });
            } else {
                return Err(Error::new_spanned(attr, "expected at most one argument"));
            }
        }
    }

    result.ok_or_else(|| Error::new_spanned(variant, "expected `route` or `subroute` attribute"))
}

fn parse_raw_url(expr: &Expr) -> syn::Result<String> {
    match expr {
        Expr::Lit(expr) => match expr.lit {
            syn::Lit::Str(ref lit_str) => Ok(lit_str.value()),
            _ => Err(Error::new_spanned(
                expr.lit.clone(),
                "expected a string literal",
            )),
        },
        _ => Err(Error::new_spanned(expr, "expected a string literal")),
    }
}

fn parse_route_url(expr: Expr) -> syn::Result<RouteUrl> {
    let url = parse_raw_url(&expr)?;

    url.parse()
        .map_err(|err: ParseError| Error::new_spanned(expr, format!("{err}\n{}", err.detail(&url))))
}

fn parse_method(expr: Expr) -> syn::Result<String> {
    match expr {
        Expr::Assign(expr) => {
            let left = match *expr.left {
                Expr::Path(expr) => expr.path.require_ident()?.to_string(),
                expr => {
                    return Err(Error::new_spanned(expr, "expected path"));
                }
            };

            match left.as_str() {
                attributes::METHOD => match *expr.right {
                    Expr::Lit(expr) => match expr.lit {
                        syn::Lit::Str(lit_str) => Ok(lit_str.value()),
                        _ => Err(Error::new_spanned(expr, "expected string literal")),
                    },
                    expr => Err(Error::new_spanned(expr, "expected path")),
                },
                _ => Err(Error::new_spanned(
                    left,
                    format!("expected `{}`", attributes::METHOD),
                )),
            }
        }
        _ => Err(Error::new_spanned(
            expr,
            format!("expected `{} = \"<GET|POST|...>\"`", attributes::METHOD),
        )),
    }
}
