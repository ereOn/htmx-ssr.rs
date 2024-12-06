//! A route URL.

use std::fmt::Display;

/// A route URL.
///
/// This type represents the URL of a route, with its optional path and query parameters.
#[derive(Debug, Clone)]
pub struct RouteUrl {
    /// The path.
    path: RouteUrlPath,

    /// The query parameters.
    query: RouteUrlQuery,
}

/// A route URL path.
#[derive(Debug, Clone)]
pub struct RouteUrlPath(Vec<RouteUrlPathSegment>);

/// A route URL path segment.
///
/// This type represents a segment of a route URL path, which can be either a slash separator, a static
/// segment, or a path parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteUrlPathSegment {
    /// A slash separator.
    Separator,

    /// A static segment.
    Static(&'static str),

    //// A path parameter.
    Parameter {
        /// The name of the parameter and its identifier.
        name: &'static str,
    },
}

/// A route URL query.
#[derive(Debug, Clone, Default)]
pub struct RouteUrlQuery(Vec<QueryParameter>);

/// A query parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueryParameter {
    /// The name of the query parameter as it appears in the URL.
    name: &'static str,

    /// The identifier that contains the value of the query parameter.
    identifier: &'static str,
}

/// An error that can occur when parsing a route URL.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// A route URL path parsing error.
    #[error(transparent)]
    Path(#[from] ParsePathError),

    /// A route URL query parsing error.
    #[error("{err}")]
    Query {
        /// The source error.
        #[source]
        err: ParseQueryError,

        /// The offset.
        offset: usize,
    },
}

impl ParseError {
    /// Returns the range of characters that caused the error.
    pub fn range(&self) -> std::ops::RangeInclusive<usize> {
        match self {
            Self::Path(err) => err.range(),
            Self::Query { err, offset } => {
                let range = err.range();

                range.start() + offset..=range.end() + offset
            }
        }
    }
}

/// An error that can occur when parsing a route URL path.
#[derive(Debug, thiserror::Error)]
pub enum ParsePathError {
    /// The route URL does not start with a slash.
    #[error("the route URL does not start with a slash")]
    NoLeadingSlash,

    /// The path contains an invalid character.
    #[error("the path contains an unexpected character (`{character}`)")]
    UnexpectedCharacter {
        /// The position at which the invalid character was found.
        position: usize,

        /// The invalid character.
        character: char,
    },

    /// A path parameter is not allowed here.
    #[error("a path parameter can only appear directly after a slash separator")]
    ParameterNotAllowed {
        /// The position at which the path parameter was found.
        position: usize,
    },

    /// A path parameter contains an invalid character.
    #[error("the path parameter contains an invalid character (`{character}`)")]
    InvalidParameterCharacter {
        /// The position at which the path parameter was opened.
        start: usize,

        /// The position at which the invalid character was found.
        position: usize,

        /// The invalid character.
        character: char,
    },

    /// A path parameter is not closed.
    #[error("the path parameter is not closed")]
    UnclosedParameter {
        /// The position at which the path parameter was opened.
        start: usize,

        /// The end position.
        end: usize,
    },
}

impl ParsePathError {
    /// Returns the range of characters that caused the error.
    pub fn range(&self) -> std::ops::RangeInclusive<usize> {
        match self {
            Self::NoLeadingSlash => 0..=0,
            Self::UnexpectedCharacter { position, .. } => *position..=*position,
            Self::ParameterNotAllowed { position } => *position..=*position,
            Self::InvalidParameterCharacter {
                start, position, ..
            } => *start..=*position,
            Self::UnclosedParameter { start, end } => *start..=*end,
        }
    }
}

/// An error that can occur when parsing a route URL query.
#[derive(Debug, thiserror::Error)]
pub enum ParseQueryError {
    /// The query parameters contain an invalid character.
    #[error(
        "the query parameters contain an unexpected character (`{character}`) at position {position}"
    )]
    UnexpectedCharacter {
        /// The position at which the invalid character was found.
        position: usize,

        /// The invalid character.
        character: char,
    },

    /// A query parameter name is not valid.
    #[error("the query parameter name `{name}` at position {position} is not a valid")]
    InvalidParameterName {
        /// The position at which the query parameter identifier was found.
        position: usize,

        /// The invalid identifier.
        name: &'static str,
    },

    /// A query parameter identifier is not a valid Rust identifier.
    #[error("the query parameter identifier `{identifier}` at position {position} is not a valid Rust identifier")]
    InvalidParameterIdentifier {
        /// The position at which the query parameter identifier was found.
        position: usize,

        /// The invalid identifier.
        identifier: &'static str,
    },
}

impl ParseQueryError {
    /// Returns the range of characters that caused the error.
    pub fn range(&self) -> std::ops::RangeInclusive<usize> {
        match self {
            Self::UnexpectedCharacter { position, .. } => *position..=*position,
            Self::InvalidParameterName { position, name } => *position..=*position + name.len(),
            Self::InvalidParameterIdentifier {
                position,
                identifier,
            } => *position..=*position + identifier.len(),
        }
    }
}

impl RouteUrl {
    /// Parses a route URL from a static string.
    pub fn from_static(s: &'static str) -> Result<Self, ParseError> {
        match s.split_once('?') {
            Some((path, query)) => {
                let path = RouteUrlPath::from_static(path)?;
                let query = RouteUrlQuery::from_static(query).map_err(|err| ParseError::Query {
                    err,
                    offset: path.to_string().len() + 1,
                })?;

                Ok(Self { path, query })
            }
            None => {
                let path = RouteUrlPath::from_static(s)?;

                Ok(Self {
                    path,
                    query: RouteUrlQuery::default(),
                })
            }
        }
    }

    /// Get the path of the route URL.
    pub fn path(&self) -> &RouteUrlPath {
        &self.path
    }

    /// Get the query of the route URL.
    pub fn query(&self) -> &RouteUrlQuery {
        &self.query
    }
}

impl Display for RouteUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.path.fmt(f)?;

        if !self.query.is_empty() {
            f.write_str("?")?;
            self.query.fmt(f)?;
        }

        Ok(())
    }
}

impl RouteUrlPath {
    /// Parses a route URL from a static string.
    pub fn from_static(s: &'static str) -> Result<Self, ParsePathError> {
        let mut chars = s.chars().enumerate();

        if chars.next() != Some((0, '/')) {
            return Err(ParsePathError::NoLeadingSlash);
        }

        let mut segments = vec![RouteUrlPathSegment::Separator];
        let mut start = None;

        while let Some((i, c)) = chars.next() {
            match c {
                '/' => {
                    if let Some(start) = start.take() {
                        segments.push(RouteUrlPathSegment::Static(&s[start..i]));
                    }

                    segments.push(RouteUrlPathSegment::Separator);
                }
                '{' => {
                    // A path parameter is only allowed after a slash separator.
                    if start.take().is_some() {
                        return Err(ParsePathError::ParameterNotAllowed { position: i });
                    }

                    if **segments.last().as_ref().unwrap() != RouteUrlPathSegment::Separator {
                        return Err(ParsePathError::ParameterNotAllowed { position: i });
                    }

                    let start = i + 1;
                    let mut stop = None;

                    for (i, c) in chars.by_ref() {
                        if c == '}' {
                            stop = Some(i);

                            break;
                        }

                        if !c.is_alphanumeric() && c != '_' {
                            return Err(ParsePathError::InvalidParameterCharacter {
                                start,
                                position: i,
                                character: c,
                            });
                        }
                    }

                    let stop = stop.ok_or(ParsePathError::UnclosedParameter {
                        start: i,
                        end: s.len() - 1,
                    })?;

                    segments.push(RouteUrlPathSegment::Parameter {
                        name: &s[start..stop],
                    });
                }
                c if is_valid_url_path_character(c) => {
                    if start.is_none() {
                        start = Some(i);
                    }
                }
                c => {
                    return Err(ParsePathError::UnexpectedCharacter {
                        position: i,
                        character: c,
                    });
                }
            }
        }

        if let Some(start) = start.take() {
            // If we still have a start, it means we have no query parameters.
            segments.push(RouteUrlPathSegment::Static(&s[start..]));

            return Ok(Self(segments));
        }

        Ok(Self(segments))
    }

    /// Returns an iterator over the path parameters.
    pub fn iter_parameters(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.0
            .iter()
            .filter_map(|segment| match segment {
                RouteUrlPathSegment::Parameter { name } => Some(name),
                _ => None,
            })
            .copied()
    }

    /// Get an Axum router path from the route URL path.
    pub fn to_axum_router_path(&self) -> String {
        // As good a guess as any...
        let mut result = String::with_capacity(64);

        for segment in &self.0 {
            match segment {
                RouteUrlPathSegment::Separator => result.push('/'),
                RouteUrlPathSegment::Static(s) => result.push_str(s),
                RouteUrlPathSegment::Parameter { name } => {
                    result.push(':');
                    result.push_str(name);
                }
            }
        }

        result
    }
}

impl Display for RouteUrlPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for segment in &self.0 {
            match segment {
                RouteUrlPathSegment::Separator => f.write_str("/")?,
                RouteUrlPathSegment::Static(s) => f.write_str(s)?,
                RouteUrlPathSegment::Parameter { name } => {
                    f.write_str("{")?;
                    f.write_str(name)?;
                    f.write_str("}")?;
                }
            }
        }

        Ok(())
    }
}

impl RouteUrlQuery {
    /// Returns whether the query is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of query parameters.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns an iterator over the query parameters.
    pub fn iter(&self) -> impl Iterator<Item = &QueryParameter> {
        self.0.iter()
    }

    /// Parses a route URL query from a static string.
    pub fn from_static(s: &'static str) -> Result<Self, ParseQueryError> {
        fn push_query_param(
            s: &'static str,
            start: usize,
            parameters: &mut Vec<QueryParameter>,
        ) -> Result<(), ParseQueryError> {
            let (name, identifier) = match s.split_once('=') {
                Some((name, identifier)) => {
                    if !is_valid_url_query_argument_name(name) {
                        return Err(ParseQueryError::InvalidParameterName {
                            position: start,
                            name,
                        });
                    }

                    if !is_valid_rust_identifier(identifier) {
                        return Err(ParseQueryError::InvalidParameterIdentifier {
                            position: start + name.len() + 1,
                            identifier,
                        });
                    }

                    (name, identifier)
                }
                None => {
                    if !is_valid_url_query_argument_name(s) {
                        return Err(ParseQueryError::InvalidParameterName {
                            position: start,
                            name: s,
                        });
                    }

                    if !is_valid_rust_identifier(s) {
                        return Err(ParseQueryError::InvalidParameterIdentifier {
                            position: start,
                            identifier: s,
                        });
                    }

                    (s, s)
                }
            };

            parameters.push(QueryParameter { name, identifier });

            Ok(())
        }

        let mut parameters = Vec::new();
        let mut start = None;

        // The remaining characters are the query parameters.
        for (i, c) in s.chars().enumerate() {
            match c {
                '&' => {
                    if let Some(start) = start.take() {
                        push_query_param(&s[start..i], start, &mut parameters)?;
                    }
                }
                c if is_valid_url_query_character(c) => {
                    if start.is_none() {
                        start = Some(i);
                    }
                }
                c => {
                    return Err(ParseQueryError::UnexpectedCharacter {
                        position: i,
                        character: c,
                    });
                }
            }
        }

        if let Some(start) = start.take() {
            push_query_param(&s[start..], start, &mut parameters)?;
        }

        Ok(Self(parameters))
    }
}

impl Display for RouteUrlQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut iter = self.0.iter();

        if let Some(parameter) = iter.next() {
            parameter.fmt(f)?;
        }

        for parameter in iter {
            f.write_str("&")?;
            parameter.fmt(f)?;
        }

        Ok(())
    }
}

impl QueryParameter {
    /// Returns the name of the query parameter as it appears in the URL.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the identifier that contains the value of the query parameter.
    pub fn identifier(&self) -> &'static str {
        self.identifier
    }
}

impl Display for QueryParameter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name)?;

        if self.name != self.identifier {
            f.write_str("=")?;
            f.write_str(self.identifier)?;
        };

        Ok(())
    }
}

/// Returns whether a character is a valid URL path character.
///
/// This method is not suited to validate a character that is part of the query parameters portion
/// of an URL. See [is_valid_url_query_character] for that.
///
/// Valid URL path characters, as per
/// [RFC3986](https://datatracker.ietf.org/doc/html/rfc3986#section-3.3) are: A–Z, a–z, 0–9, -, .,
/// _, ~, !, $, &, ', (, ), *, +, ,, ;, =, :, @, as well as % and /.
fn is_valid_url_path_character(c: char) -> bool {
    matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' | '!' | '$' | '&' | '\''
        | '(' | ')' | '*' | '+' | ',' | ';' | '=' | ':' | '@' | '%' | '/')
}

/// Returns whether a character is a valid URL query character.
///
/// Valid URL query characters, as per
/// [RFC3986](https://datatracker.ietf.org/doc/html/rfc3986#section-3.4) are: A–Z, a–z, 0–9, -, .,
/// _, ~.
fn is_valid_url_query_character(c: char) -> bool {
    matches!(c, 'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' | '=')
}

/// Returns whether a string is a valid URL query argument name.
///
/// Valid URL query argument names are [A-Za-z_][A-Za-z0-9_-]*.
fn is_valid_url_query_argument_name(s: &str) -> bool {
    let mut chars = s.chars();

    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Returns whether a string is a valid Rust identifier.
///
/// Valid Rust identifiers are [a-zA-Z_][a-zA-Z0-9_]*.
fn is_valid_rust_identifier(s: &str) -> bool {
    let mut chars = s.chars();

    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }

    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_route_url() {
        let u = RouteUrl::from_static("/").unwrap();
        assert_eq!(u.to_string(), "/");
        assert!(u.query().is_empty());
        assert!(u.path().iter_parameters().next().is_none());
        assert_eq!(u.path().to_axum_router_path(), "/");

        let u = RouteUrl::from_static("/foo").unwrap();
        assert_eq!(u.to_string(), "/foo");
        assert!(u.query().is_empty());
        assert!(u.path().iter_parameters().next().is_none());
        assert_eq!(u.path().to_axum_router_path(), "/foo");

        let u = RouteUrl::from_static("/foo/{bar}").unwrap();
        assert_eq!(u.to_string(), "/foo/{bar}");
        assert!(u.query().is_empty());
        assert_eq!(u.path().iter_parameters().collect::<Vec<_>>(), vec!["bar"]);
        assert_eq!(u.path().to_axum_router_path(), "/foo/:bar");

        let u = RouteUrl::from_static("/user/{uid}/comment/{cid}").unwrap();
        assert_eq!(u.to_string(), "/user/{uid}/comment/{cid}");
        assert!(u.query().is_empty());
        assert_eq!(
            u.path().iter_parameters().collect::<Vec<_>>(),
            vec!["uid", "cid"]
        );
        assert_eq!(u.path().to_axum_router_path(), "/user/:uid/comment/:cid");

        let u = RouteUrl::from_static("/user/{uid}/comment/{cid}?").unwrap();
        assert_eq!(u.to_string(), "/user/{uid}/comment/{cid}");
        assert!(u.query().is_empty());
        assert_eq!(
            u.path().iter_parameters().collect::<Vec<_>>(),
            vec!["uid", "cid"]
        );
        assert_eq!(u.path().to_axum_router_path(), "/user/:uid/comment/:cid");

        let u =
            RouteUrl::from_static("/user/{uid}/comment/{cid}?limit&page=offset&sort=sort").unwrap();
        assert_eq!(
            u.to_string(),
            "/user/{uid}/comment/{cid}?limit&page=offset&sort"
        );
        assert_eq!(
            u.query().iter().collect::<Vec<_>>(),
            &[
                &QueryParameter {
                    name: "limit",
                    identifier: "limit"
                },
                &QueryParameter {
                    name: "page",
                    identifier: "offset"
                },
                &QueryParameter {
                    name: "sort",
                    identifier: "sort"
                },
            ]
        );
        assert_eq!(
            u.path().iter_parameters().collect::<Vec<_>>(),
            vec!["uid", "cid"]
        );
        assert_eq!(u.path().to_axum_router_path(), "/user/:uid/comment/:cid");
    }

    #[test]
    fn test_parse_route_url_no_leading_slash() {
        let err = RouteUrl::from_static("foo").unwrap_err();

        match err {
            ParseError::Path(ParsePathError::NoLeadingSlash) => {}
            _ => panic!("unexpected error: {err:?}"),
        }
    }

    #[test]
    fn test_parse_route_url_unexpected_character() {
        let err = RouteUrl::from_static("/foo</bar").unwrap_err();

        match err {
            ParseError::Path(ParsePathError::UnexpectedCharacter {
                position,
                character,
            }) => {
                assert_eq!(position, 4);
                assert_eq!(character, '<');
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 4..=4);
    }

    #[test]
    fn test_parse_route_url_invalid_parameter_character() {
        let err = RouteUrl::from_static("/foo/{bar<}").unwrap_err();

        match err {
            ParseError::Path(ParsePathError::InvalidParameterCharacter {
                start,
                position,
                character,
            }) => {
                assert_eq!(start, 6);
                assert_eq!(position, 9);
                assert_eq!(character, '<');
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 6..=9);
    }

    #[test]
    fn test_parse_route_url_parameter_not_allowed() {
        let err = RouteUrl::from_static("/foo/prefix-{bar}").unwrap_err();

        match err {
            ParseError::Path(ParsePathError::ParameterNotAllowed { position }) => {
                assert_eq!(position, 12);
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 12..=12);
    }

    #[test]
    fn test_parse_route_url_parameter_not_allowed_twice() {
        let err = RouteUrl::from_static("/foo/{foo}{bar}").unwrap_err();

        match err {
            ParseError::Path(ParsePathError::ParameterNotAllowed { position }) => {
                assert_eq!(position, 10);
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 10..=10);
    }

    #[test]
    fn test_parse_route_url_unclosed_parameter() {
        let err = RouteUrl::from_static("/foo/{bar").unwrap_err();

        match err {
            ParseError::Path(ParsePathError::UnclosedParameter { start, end }) => {
                assert_eq!(start, 5);
                assert_eq!(end, 8);
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 5..=8);
    }

    #[test]
    fn test_parse_route_url_query_unexpected_character() {
        let err = RouteUrl::from_static("/foo?bar<").unwrap_err();

        match err {
            ParseError::Query {
                err:
                    ParseQueryError::UnexpectedCharacter {
                        position,
                        character,
                    },
                offset,
            } => {
                assert_eq!(position, 3);
                assert_eq!(character, '<');
                assert_eq!(offset, 5);
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 8..=8);
    }

    #[test]
    fn test_parse_route_url_query_invalid_parameter_name() {
        let err = RouteUrl::from_static("/foo?x&0bar=").unwrap_err();

        match err {
            ParseError::Query {
                err: ParseQueryError::InvalidParameterName { position, name },
                offset,
            } => {
                assert_eq!(position, 2);
                assert_eq!(name, "0bar");
                assert_eq!(offset, 5);
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 7..=11);
    }

    #[test]
    fn test_parse_route_url_query_invalid_parameter_identifier() {
        let err = RouteUrl::from_static("/foo?x&b-ar").unwrap_err();

        match err {
            ParseError::Query {
                err:
                    ParseQueryError::InvalidParameterIdentifier {
                        position,
                        identifier,
                    },
                offset,
            } => {
                assert_eq!(position, 2);
                assert_eq!(identifier, "b-ar");
                assert_eq!(offset, 5);
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 7..=11);
    }

    #[test]
    fn test_parse_route_url_query_invalid_parameter_identifier_different_name() {
        let err = RouteUrl::from_static("/foo?x&bar=0baz").unwrap_err();

        match err {
            ParseError::Query {
                err:
                    ParseQueryError::InvalidParameterIdentifier {
                        position,
                        identifier,
                    },
                offset,
            } => {
                assert_eq!(position, 6);
                assert_eq!(identifier, "0baz");
                assert_eq!(offset, 5);
            }
            _ => panic!("unexpected error: {err:?}"),
        }

        assert_eq!(err.range(), 11..=15);
    }
}
