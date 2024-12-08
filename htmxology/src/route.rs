//! The route trait.

use std::fmt::Display;

/// The route trait can be implemented for types that represent a possible set of routes in an
/// application.
pub trait Route: Display {
    /// Register the routes of the controller into the specified Axum router.
    fn make_router<Controller: super::Controller<Route = Self>>(
    ) -> axum::Router<crate::State<Controller::Model>>;
}
