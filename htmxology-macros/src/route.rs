//! Route derive macro.

use std::{collections::BTreeMap, fmt::Write};

use quote::{quote, quote_spanned, ToTokens};
use syn::{punctuated::Punctuated, spanned::Spanned, Data, Error, Expr, Fields, Ident, Token};

mod attributes {
    pub(super) const URL: &str = "url";
    pub(super) const PATH: &str = "path";
    pub(super) const METHOD: &str = "method";
    pub(super) const QUERY: &str = "query";
}

struct RouteInfo {
    url: Url,
    method: Ident,
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

        let mut route_infos = Vec::with_capacity(variant.attrs.len());

        for attr in &variant.attrs {
            if attr.path().is_ident(attributes::URL) {
                let exprs =
                    attr.parse_args_with(Punctuated::<syn::Expr, Token![,]>::parse_terminated)?;

                let route_info = parse_url_attribute(ident, exprs)?;
                route_infos.push(route_info);
            }
        }

        let route_info = route_infos
            .first()
            .ok_or_else(|| Error::new_spanned(variant, "expected `url` attribute"))?;

        let handler = match &variant.fields {
            // Enum::Unit
            Fields::Unit => {
                let RouteInfo { url, .. } = route_info;
                let url = url.to_unparametered_string(variant)?;

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
            // Enum::Named{}
            Fields::Named(fields) if fields.named.is_empty() => {
                let RouteInfo { url, .. } = route_info;
                let url = url.to_unparametered_string(variant)?;

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
            // Enum::Unnamed()
            Fields::Unnamed(fields) if fields.unnamed.is_empty() => {
                let RouteInfo { url, .. } = route_info;
                let url = url.to_unparametered_string(variant)?;

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
                let mut args = Vec::with_capacity(fields.named.len());
                let mut args_defs = Vec::with_capacity(fields.named.len());
                let mut path_args = Vec::with_capacity(fields.named.len());
                let mut path_args_names = BTreeMap::new();
                let mut query_args = Vec::with_capacity(fields.named.len());
                let mut query_args_names = BTreeMap::new();
                let mut query_args_defs = Vec::with_capacity(fields.named.len());

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

                    if is_query {
                        query_args.push(quote_spanned! { field_ident.span() =>
                            #field_ident
                        });
                        query_args_defs.push(quote_spanned! { field_ident.span() =>
                            #[serde(default)]
                            #field_ident: #field_ty
                        });
                        query_args_names.insert(field_ident.to_string(), field_ident.clone());
                    } else {
                        path_args.push(quote_spanned! { field_ident.span() =>
                            #field_ident
                        });
                        path_args_names.insert(field_ident.to_string(), field_ident.clone());
                    }
                }

                let url = &route_info.url;

                let url = if path_args_names.is_empty() {
                    url.to_unparametered_string(variant)?
                } else {
                    url.to_named_parameters_format(variant, path_args_names)?
                };

                let url = if query_args_names.is_empty() {
                    url
                } else {
                    let mut queries = Vec::with_capacity(query_args_names.len());

                    for (name, ident) in query_args_names {
                        queries.push(quote_spanned! { ident.span() =>
                            if let Some(#ident) = #ident {
                                query_args.push(format!(concat!(#name, "={}"), urlencoding::encode(&#ident.to_string())));
                            }
                        });
                    }

                    let queries_len = queries.len();

                    quote! {
                        {
                            let url = #url;

                            let mut query_args = Vec::with_capacity(#queries_len);

                            #(#queries)*

                            if query_args.is_empty() {
                                url
                            } else {
                                format!("{url}?{}", query_args.join("&"))
                            }
                        }
                    }
                };

                to_urls.push(quote! {
                    Self::#ident{#(#args),*} => #url
                });

                let params = Ident::new(&format!("{root_ident}{ident}Params"), ident.span());

                let path_parse = if path_args.is_empty() {
                    quote!()
                } else {
                    quote! {
                        let axum::extract::Path((#(#path_args),*)) = axum::extract::Path::from_request_parts(parts, state)
                            .await
                            .map_err(|err| err.into_response())?;
                    }
                };

                let query_parse = if query_args.is_empty() {
                    quote!()
                } else {
                    quote! {
                        let axum::extract::Query(query) =
                        axum::extract::Query::from_request_parts(parts, state)
                            .await
                            .map_err(|err| err.into_response())?;

                        #[derive(serde::Deserialize)]
                        struct Query {
                            #(#query_args_defs),*
                        }

                        let Query { #(#query_args),* } = query;
                    }
                };

                declarations.push(quote! {
                    struct #params {
                        #(#args_defs),*
                    }

                    #[axum::async_trait]
                    impl<S: Send + Sync> axum::extract::FromRequestParts<S> for #params {
                        type Rejection = axum::response::Response;

                        async fn from_request_parts(
                            parts: &mut http::request::Parts,
                            state: &S,
                        ) -> Result<Self, Self::Rejection> {
                            #path_parse
                            #query_parse

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
                let params = fields.unnamed.iter().map(|field| {
                    let ty = &field.ty;
                    quote_spanned! {field.span() => #ty}
                });
                let args = fields
                    .unnamed
                    .iter()
                    .enumerate()
                    .map(|(i, field)| Ident::new(&format!("arg{i}"), field.span()))
                    .collect::<Vec<_>>();
                let url = route_info
                    .url
                    .to_unnamed_parameters_format(variant, args.clone())?;

                to_urls.push(quote! {
                    Self::#ident(#(#args)*,) => #url
                });
                let params = quote_spanned! {fields.span() => (#(#params),*)};

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

        for RouteInfo { url, method } in &route_infos {
            let url = url.to_axum_route_path(variant)?;

            routes.push(quote! {
                .route(#url, axum::routing::#method(#handler))
            });
        }
    }

    Ok(quote! {
        #(#declarations)*

        impl htmxology::Route for #root_ident {
            fn register_routes<Controller: htmxology::Controller<Route=Self>>(
                router: axum::Router<htmxology::State<Controller::Model>>,
            ) -> axum::Router<htmxology::State<Controller::Model>> {
                router
                    #(#routes)*
            }

            fn to_url(&self) -> String {
                match self {
                    #(#to_urls),*
                }
            }
        }

        impl std::fmt::Display for #root_ident {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.to_url())
            }
        }
    })
}

fn parse_url_attribute(
    ident: &Ident,
    exprs: impl IntoIterator<Item = Expr>,
) -> syn::Result<RouteInfo> {
    let mut exprs = exprs.into_iter();

    let first = match exprs.next() {
        Some(first) => first,
        None => {
            return Err(Error::new_spanned(ident, "expected at least one argument"));
        }
    };

    let url = match first {
        Expr::Lit(expr) => match expr.lit {
            syn::Lit::Str(lit_str) => Url::from_str(&lit_str, &lit_str.value())?,
            _ => {
                return Err(Error::new_spanned(expr.lit, "expected a string literal"));
            }
        },
        Expr::Assign(expr) => {
            let left = match *expr.left {
                Expr::Path(expr) => expr.path.require_ident()?.to_string(),
                expr => {
                    return Err(Error::new_spanned(expr, "expected path"));
                }
            };

            if left != attributes::PATH {
                return Err(Error::new_spanned(left, "expected `path`"));
            }

            match *expr.right {
                Expr::Lit(expr) => match expr.lit {
                    syn::Lit::Str(lit_str) => Url::from_str(&lit_str, &lit_str.value())?,
                    _ => {
                        return Err(Error::new_spanned(expr.lit, "expected a string literal"));
                    }
                },
                _ => {
                    return Err(Error::new_spanned(expr.right, "expected a string literal"));
                }
            }
        }
        _ => {
            return Err(Error::new_spanned(first, "expected a string literal"));
        }
    };

    let mut method = Ident::new("get", ident.span());

    for expr in exprs {
        match expr {
            Expr::Assign(expr) => {
                let left = match *expr.left {
                    Expr::Path(expr) => expr.path.require_ident()?.to_string(),
                    expr => {
                        return Err(Error::new_spanned(expr, "expected path"));
                    }
                };

                match left.as_str() {
                    attributes::METHOD => {
                        method = match *expr.right {
                            Expr::Path(expr) => expr.path.require_ident()?.clone(),
                            expr => {
                                return Err(Error::new_spanned(expr, "expected path"));
                            }
                        };
                    }
                    _ => {
                        return Err(Error::new_spanned(left, "expected `method`"));
                    }
                }
            }
            _ => {
                return Err(Error::new_spanned(expr, "expected `<foo> = <bar>`"));
            }
        }
    }

    Ok(RouteInfo { url, method })
}

/// An URL, as specified in the `url` attribute.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Url(Vec<UrlElement>);

impl Url {
    /// Parses an URL from a string.
    ///
    /// The URL must be a valid ASCII string and may contain named `{name}` or unnamed `{}` path
    /// arguments.
    fn from_str<T: ToTokens>(t: &T, s: &str) -> syn::Result<Self> {
        if !s.is_ascii() {
            return Err(Error::new_spanned(t, "URL must be ASCII"));
        }

        if !s.starts_with('/') {
            return Err(Error::new_spanned(t, "URL must start with `/`"));
        }

        let mut elements = vec![UrlElement::Slash];

        let mut i = 1;
        let mut last = i;
        let bytes = s.as_bytes();

        while i < bytes.len() {
            let c = bytes[i];

            match c {
                b'/' => {
                    if last < i {
                        let literal =
                            String::from_utf8((bytes[last..i]).to_vec()).expect("valid UTF-8");

                        elements.push(UrlElement::Literal(literal));
                    }

                    elements.push(UrlElement::Slash);

                    i += 1;
                    last = i;
                }
                b'{' => {
                    if last < i {
                        let literal =
                            String::from_utf8((bytes[last..i]).to_vec()).expect("valid UTF-8");

                        elements.push(UrlElement::Literal(literal));
                    }

                    let start = i + 1;

                    let mut end = start;

                    while end < bytes.len() && bytes[end] != b'}' {
                        match bytes[end] {
                            b'_' | b'a'..=b'z' | b'A'..=b'Z' => {
                                end += 1;
                            }
                            // Allow digits in the middle of the name, but not at the start.
                            b'0'..=b'9' if end > start => {
                                end += 1;
                            }
                            c => {
                                return Err(Error::new_spanned(
                                    t,
                                    format!("invalid character `{c}` in path parameter name"),
                                ));
                            }
                        };
                    }

                    if end == bytes.len() {
                        return Err(Error::new_spanned(t, "unmatched `{`"));
                    }

                    let name = String::from_utf8((bytes[start..end]).to_vec())
                        .map_err(|_| Error::new_spanned(t, "invalid UTF-8"))?;

                    if name.is_empty() {
                        elements.push(UrlElement::UnnamedPathParameter);
                    } else {
                        elements.push(UrlElement::NamedPathParameter(name));
                    }

                    i = end + 1;
                    last = i;
                }
                b' ' | b'\t' | b'\r' | b'\n' | b'}' | b'<' | b'>' | b'^' => {
                    return Err(Error::new_spanned(
                        t,
                        format!("invalid character `{c}` in URL"),
                    ));
                }
                _ => {
                    i += 1;
                }
            }
        }

        if last < i {
            let literal = String::from_utf8((bytes[last..i]).to_vec())
                .map_err(|err| Error::new_spanned(t, format!("invalid UTF-8: {err}")))?;

            elements.push(UrlElement::Literal(literal));
        }

        Ok(Self(elements))
    }

    /// Get an Axum route path from the URL.
    fn to_axum_route_path<T: ToTokens>(&self, t: &T) -> syn::Result<String> {
        let mut path = String::with_capacity(64);
        let mut unnamed_path_arg_names = (0..).map(|i| format!("arg{}", i));
        let mut last_was_slash = false;

        for element in &self.0 {
            match element {
                UrlElement::Slash => {
                    path.push('/');
                    last_was_slash = true;
                }
                UrlElement::Literal(s) => {
                    path.push_str(s);
                    last_was_slash = false;
                }
                UrlElement::NamedPathParameter(s) if last_was_slash => {
                    write!(path, ":{s}").expect("writing to string cannot fail");
                    last_was_slash = false;
                }
                UrlElement::UnnamedPathParameter if last_was_slash => {
                    let name = unnamed_path_arg_names
                        .next()
                        .ok_or_else(|| Error::new_spanned(t, "ran out of path argument names"))?;

                    path.push_str(&format!(":{name}"));

                    last_was_slash = false;
                }
                _ => {
                    return Err(Error::new_spanned(
                        t,
                        "path parameters must be preceded by a slash when using Axum routing",
                    ));
                }
            }
        }

        Ok(path)
    }

    /// Get the URL as a string, failing if there are any required path parameters.
    fn to_unparametered_string(&self, t: &impl ToTokens) -> syn::Result<proc_macro2::TokenStream> {
        let mut s = String::with_capacity(64);

        for element in &self.0 {
            match element {
                UrlElement::Slash => s.push('/'),
                UrlElement::Literal(literal) => s.push_str(literal),
                UrlElement::NamedPathParameter(name) => {
                    return Err(Error::new_spanned(
                        t,
                        format!("the URL contains named path parameter `{name}`, which cannot be formatted without parameters"),
                    ));
                }
                UrlElement::UnnamedPathParameter => {
                    return Err(Error::new_spanned(
                        t,
                        "the URL contains unnamed path parameters, which cannot be formatted without parameters",
                    ));
                }
            }
        }

        Ok(quote! {#s.to_owned()})
    }

    /// Get the URL as a format string, with its name path parameters resolved.
    fn to_named_parameters_format(
        &self,
        t: &impl ToTokens,
        name_params: impl IntoIterator<Item = (String, Ident)>,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let mut s = String::with_capacity(64);
        let mut name_params = name_params.into_iter().collect::<BTreeMap<_, _>>();
        let has_params = !name_params.is_empty();

        for element in &self.0 {
            match element {
                UrlElement::Slash => s.push('/'),
                UrlElement::Literal(literal) => s.push_str(literal),
                UrlElement::NamedPathParameter(name) => {
                    let ident = name_params.remove(name.as_str()).ok_or_else(|| {
                        Error::new_spanned(t, format!("missing named path parameter `{name}`"))
                    })?;

                    s.push_str(&format!("{{{ident}}}"));
                }
                UrlElement::UnnamedPathParameter => {
                    return Err(Error::new_spanned(
                        t,
                        "the URL contains unnamed path parameters, which cannot be formatted from named path parameters",
                    ));
                }
            }
        }

        if let Some((name, ident)) = name_params.into_iter().next() {
            return Err(Error::new_spanned(
                ident,
                format!(
                    "the URL ({s}) does not contain all named path parameters (missing: {name})"
                ),
            ));
        }

        if has_params {
            Ok(quote! {format!(#s)})
        } else {
            Ok(quote! {#s.to_owned()})
        }
    }

    /// Get the URL as a format string, with its unnamed path parameters resolved.
    fn to_unnamed_parameters_format(
        &self,
        t: &impl ToTokens,
        unnamed_params: impl IntoIterator<Item = Ident>,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let mut s = String::with_capacity(64);
        let mut unnamed_params = unnamed_params.into_iter();

        for element in &self.0 {
            match element {
                UrlElement::Slash => s.push('/'),
                UrlElement::Literal(literal) => s.push_str(literal),
                UrlElement::NamedPathParameter(_) | UrlElement::UnnamedPathParameter => {
                    let ident = unnamed_params.next().ok_or_else(|| {
                        Error::new_spanned(
                            t,
                            "the URL contains more path parameters than route has arguments",
                        )
                    })?;

                    s.push_str(&format!("{{{ident}}}"));
                }
            }
        }

        if let Some(ident) = unnamed_params.next() {
            return Err(Error::new_spanned(
                ident,
                "the URL does not contain all unnamed path parameters",
            ));
        }

        Ok(quote! {format!(#s)})
    }
}

impl std::fmt::Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for element in &self.0 {
            write!(f, "{element}")?;
        }

        Ok(())
    }
}

/// An element of a URL.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum UrlElement {
    /// A slash separator.
    Slash,

    /// A literal string.
    Literal(String),

    /// A named path parameter.
    NamedPathParameter(String),

    /// An unnamed path parameter.
    UnnamedPathParameter,
}

impl std::fmt::Display for UrlElement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UrlElement::Slash => f.write_char('/'),
            UrlElement::Literal(s) => f.write_str(s),
            UrlElement::NamedPathParameter(s) => write!(f, "{{{s}}}"),
            UrlElement::UnnamedPathParameter => f.write_str("{}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::route::UrlElement;
    use proc_macro2::Span;
    use syn::Ident;

    use super::Url;

    #[test]
    fn test_url_root() {
        let s = "/";
        let url = Url::from_str(&s, s).unwrap();

        assert_eq!(url, Url(vec![UrlElement::Slash]));
        assert_eq!(url.to_string(), "/");
        assert_eq!(url.to_axum_route_path(&s).unwrap(), "/");
        assert_eq!(
            url.to_named_parameters_format(&s, []).unwrap().to_string(),
            r#""/" . to_owned ()"#
        );
    }

    #[test]
    fn test_url_without_args() {
        let s = "/some/nice/url";
        let url = Url::from_str(&s, s).unwrap();

        assert_eq!(
            url,
            Url(vec![
                UrlElement::Slash,
                UrlElement::Literal("some".to_string()),
                UrlElement::Slash,
                UrlElement::Literal("nice".to_string()),
                UrlElement::Slash,
                UrlElement::Literal("url".to_string()),
            ])
        );
        assert_eq!(url.to_string(), "/some/nice/url");
        assert_eq!(url.to_axum_route_path(&s).unwrap(), "/some/nice/url");
        assert_eq!(
            url.to_named_parameters_format(&s, []).unwrap().to_string(),
            r#""/some/nice/url" . to_owned ()"#
        );
    }

    #[test]
    fn test_url_with_named_arguments() {
        let s = "/fruits/{fruit}/colors/{color}/";
        let url = Url::from_str(&s, s).unwrap();

        assert_eq!(
            url,
            Url(vec![
                UrlElement::Slash,
                UrlElement::Literal("fruits".to_string()),
                UrlElement::Slash,
                UrlElement::NamedPathParameter("fruit".to_string()),
                UrlElement::Slash,
                UrlElement::Literal("colors".to_string()),
                UrlElement::Slash,
                UrlElement::NamedPathParameter("color".to_string()),
                UrlElement::Slash,
            ])
        );
        assert_eq!(url.to_string(), "/fruits/{fruit}/colors/{color}/");
        assert_eq!(
            url.to_axum_route_path(&s).unwrap(),
            "/fruits/:fruit/colors/:color/"
        );
        assert_eq!(
            url.to_named_parameters_format(
                &s,
                [
                    ("fruit".to_owned(), Ident::new("myfruit", Span::call_site())),
                    ("color".to_owned(), Ident::new("mycolor", Span::call_site()))
                ]
            )
            .unwrap()
            .to_string(),
            r#"format ! ("/fruits/{myfruit}/colors/{mycolor}/")"#
        );
        assert_eq!(
            url.to_unnamed_parameters_format(
                &s,
                [
                    Ident::new("myfruit", Span::call_site()),
                    Ident::new("mycolor", Span::call_site())
                ]
            )
            .unwrap()
            .to_string(),
            r#"format ! ("/fruits/{myfruit}/colors/{mycolor}/")"#
        );
    }

    #[test]
    fn test_url_with_unnamed_arguments() {
        let s = "/fruits/{}/colors/{}/";
        let url = Url::from_str(&s, s).unwrap();

        assert_eq!(
            url,
            Url(vec![
                UrlElement::Slash,
                UrlElement::Literal("fruits".to_string()),
                UrlElement::Slash,
                UrlElement::UnnamedPathParameter,
                UrlElement::Slash,
                UrlElement::Literal("colors".to_string()),
                UrlElement::Slash,
                UrlElement::UnnamedPathParameter,
                UrlElement::Slash,
            ])
        );
        assert_eq!(url.to_string(), "/fruits/{}/colors/{}/");
        assert_eq!(
            url.to_axum_route_path(&s).unwrap(),
            "/fruits/:arg0/colors/:arg1/"
        );
        assert_eq!(
            url.to_unnamed_parameters_format(
                &s,
                [
                    Ident::new("myfruit", Span::call_site()),
                    Ident::new("mycolor", Span::call_site())
                ]
            )
            .unwrap()
            .to_string(),
            r#"format ! ("/fruits/{myfruit}/colors/{mycolor}/")"#
        );
    }

    #[test]
    fn test_invalid_url_non_ascii() {
        let s = "/fruits/🍎";
        Url::from_str(&s, s).unwrap_err();
    }

    #[test]
    fn test_invalid_url_doesnt_start_with_slash() {
        let s = "fruits/banana";
        Url::from_str(&s, s).unwrap_err();
    }

    #[test]
    fn test_invalid_url_invalid_path_arg_name() {
        let s = "/fruits/{0fruit}";
        Url::from_str(&s, s).unwrap_err();
    }

    #[test]
    fn test_invalid_url_unmatched_path_arg() {
        let s = "/fruits/{fruit/";
        Url::from_str(&s, s).unwrap_err();
    }

    #[test]
    fn test_invalid_url_invalid_char() {
        let s = "/fruits/}";
        Url::from_str(&s, s).unwrap_err();
    }

    #[test]
    fn test_url_with_invalid_axum_route_path() {
        let s = "/fruits/fruit-{fruit}/taste";
        let url = Url::from_str(&s, s).unwrap();

        url.to_axum_route_path(&s).unwrap_err();
    }

    #[test]
    fn test_url_without_named_parameters_placeholders() {
        let s = "/fruits/{}/taste";
        let url = Url::from_str(&s, s).unwrap();

        url.to_named_parameters_format(
            &s,
            [("fruit".to_owned(), Ident::new("myfruit", Span::call_site()))],
        )
        .unwrap_err();
    }

    #[test]
    fn test_url_with_missing_named_parameters() {
        let s = "/fruits/{fruit}/color/{color}";
        let url = Url::from_str(&s, s).unwrap();

        url.to_named_parameters_format(
            &s,
            [("fruit".to_owned(), Ident::new("myfruit", Span::call_site()))],
        )
        .unwrap_err();
    }

    #[test]
    fn test_url_with_extra_named_parameters() {
        let s = "/fruits/{fruit}/color/{color}";
        let url = Url::from_str(&s, s).unwrap();

        url.to_named_parameters_format(
            &s,
            [
                ("fruit".to_owned(), Ident::new("myfruit", Span::call_site())),
                ("color".to_owned(), Ident::new("mycolor", Span::call_site())),
                ("name".to_owned(), Ident::new("myname", Span::call_site())),
            ],
        )
        .unwrap_err();
    }

    #[test]
    fn test_url_with_missing_unnamed_parameters() {
        let s = "/fruits/{fruit}/color/{color}";
        let url = Url::from_str(&s, s).unwrap();

        url.to_unnamed_parameters_format(&s, [Ident::new("myfruit", Span::call_site())])
            .unwrap_err();
    }

    #[test]
    fn test_url_with_extra_unnamed_parameters() {
        let s = "/fruits/{fruit}/color/{color}";
        let url = Url::from_str(&s, s).unwrap();

        url.to_unnamed_parameters_format(
            &s,
            [
                Ident::new("myfruit", Span::call_site()),
                Ident::new("mycolor", Span::call_site()),
                Ident::new("myname", Span::call_site()),
            ],
        )
        .unwrap_err();
    }
}
