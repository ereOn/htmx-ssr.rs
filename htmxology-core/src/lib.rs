//! Htmxology core types.
//!
//! This crate contains the core types and traits for the Htmxology library, that are used by the
//! higher-level `htmxology` and `htmxology-macros` crates.

mod route_method;
mod route_url;

pub use route_method::RouteMethod;
pub use route_url::{
    ParseError as RouteUrlParseError, QueryParameter, RouteUrl, RouteUrlPath, RouteUrlQuery,
};
