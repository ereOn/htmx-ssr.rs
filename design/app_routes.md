# App Routes

This document describes the feature of "App Routes" in the library.

## Introduction

This features assumes that web applications have finite set of routes, which
can be partially parametric.

The idea of the feature is to map all the routes of the application to a Rust
enum, whose each variant represents a route.

A route is not just a URL but a combination of URL and HTTP method, as well as
optional parameters.

There are two kinds of parameters:

- Path parameters: extracted from the URL path, they are defined by their position in the route.
- Query parameters: extracted from the URL query string, they are defined by their names.

## Example

The following example shows how to define a set of routes for a simple web application, and covers all the possible cases.

```rust
/// The enum that represents the routes of the application.
///
/// All routes MUST start with a `/` character and can't end with one.
#[derive(Route)]
enum AppRoute {
    /// A route with no parameters.
    #[get("/")]
    Home,

    /// A route with no parameters, as an empty tuple variant. This is not
    /// recommended but nevertheless supported.
    #[get("/about")]
    About(),

    /// A route with no parameters, as an empty struct variant. This is not
    /// recommended but nevertheless supported.
    #[get("/contact")]
    Contact {},

    /// A route with a single path parameter, as struct variant.
    ///
    /// In this case all the named parameters must exist in the struct variant,
    /// and all of them have to be used exactly once. Their order does not
    /// matter.
    #[get("/user/{id}")]
    User { id: u32 },

    /// A route with a single path parameter, as a tuple variant.
    ///
    /// In this case there must be exactly as many parameters as there are
    /// tuple fields.
    #[get("/product/{id}")]
    Product(u32),

    /// A route with multiple path parameters and query parameters.
    ///
    /// The fields used in the query parameters must implement `Default`.
    #[get("/user/search/{query}?sort={sort}&page={page}&filter={filter}")]
    UserSearch {
        query: String,
        sort: Sort,
        page: u32,
        filter: Option<String>,
    },

    /// A route with multiple path parameters and query parameters, as a tuple variant.
    ///
    /// The fields used in the query parameters must implement `Default`. All
    /// the fields must be used exactly once and appear in the same order as in the
    /// route.
    ///
    /// Query parameters can be specified using the `name=identifier` syntax or
    /// the shorthand version `name`. In the shorthand version the identifier
    /// name is assumed to be the same as the parameter name.
    ///
    /// Identifiers names must be valid Rust identifiers regardless of the
    /// syntax used.
    #[get("/products/search/{query}?sort={sort}&page&filter={filter}")]
    ProductSearch(String, Sort, u32, Option<String>),

    /// A route with a different HTTP method.
    #[post("/user/{id}/profile")]
    UpdateUserProfile { id: u32 },

    /// A route with a different HTTP method and query parameters.
    #[delete("/user/{id}/profile?archive={archive}")]
    DeleteUserProfile(u32, bool),

    /// A special sub-route variant that allows for better decoupling of the
    /// routes in an application.
    ///
    /// The sub-route uses the `/admin` prefix such that all the routes it
    /// exposes will start with `/admin`.
    ///
    /// The only tuple field is the sub-route enum type, and must use the
    /// `#[derive(Route)]` attribute.
    #[subroute("/admin")]
    Admin(AdminAppRoute),

    /// Another sub-route variant with path parameters.
    ///
    /// The sub-route uses the `/user/{id}/profile` prefix such that all the
    /// routes it exposes will start with `/user/{id}/profile`.
    ///
    /// The last field is the sub-route enum type, and must use the
    /// `#[derive(Route)]` attribute.
    ///
    /// No query parameters are allowed in parent sub-routes enum variants.
    /// They are of course allowed in the sub-route enum variants.
    ///
    /// The sub-route can never use named arguments that are used in of the
    /// parent route.
    #[subroute("/user/{id}/profile")]
    UserProfile { id: u32, subroute: UserProfileRoute },

    /// A route with a body.
    ///
    /// If there is one field with a `#[body]` attribute, it is assumed to be
    /// the body of the request.
    ///
    /// Depending on the specified media type, the body will be deserialized in
    /// different ways.
    ///
    /// Supported media types are `application/json` and
    /// `application/x-www-form-urlencoded`.
    #[put("/user/{id}/profile")]
    UpdateUserProfile { 
        id: u32,

        #[body("application/json")]
        body: UserProfile,
    },

    /// A route with a body and query parameters.
    #[post("/user/{id}/profile?create={create}")]
    CreateUserProfile(
        u32,
        bool,
        #[body("application/x-www-form-urlencoded")] UserProfile,
    ),
}

/// The enum that represents the routes of the admin section of the application.
#[derive(Route)]
enum AdminAppRoute {
    /// A route with no parameters.
    #[get("/")]
    Home,

    // Other routes...
}
```
