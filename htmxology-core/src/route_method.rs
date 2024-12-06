//! A route method.

/// A route method.
///
/// This is a subset of the standard HTTP methods, that are commonly used in web applications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RouteMethod {
    /// The `GET` method.
    Get,

    /// The `POST` method.
    Post,

    /// The `PUT` method.
    Put,

    /// The `DELETE` method.
    Delete,

    /// The `PATCH` method.
    Patch,
}

impl RouteMethod {
    /// Get the axum-compatible router method name.
    pub fn axum_router_method_name(&self) -> &'static str {
        match self {
            RouteMethod::Get => "get",
            RouteMethod::Post => "post",
            RouteMethod::Put => "put",
            RouteMethod::Delete => "delete",
            RouteMethod::Patch => "patch",
        }
    }
}
