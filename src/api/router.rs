//! Router helpers for Subsonic API endpoints.
//!
//! Provides utilities for registering endpoints with common patterns like
//! automatic .view suffix handling and GET+POST method support.

use axum::{Router, handler::Handler, routing::get};

/// Extension trait for Router to simplify Subsonic API route registration.
pub trait SubsonicRouterExt<S>
where
    S: Clone + Send + Sync + 'static,
{
    /// Register a Subsonic API endpoint with automatic .view suffix.
    /// Both the base path and path.view will be registered with GET and POST methods.
    ///
    /// # Example
    /// ```ignore
    /// let router = Router::new()
    ///     .subsonic_route("/ping", handlers::ping)
    ///     .subsonic_route("/getLicense", handlers::get_license);
    /// ```
    ///
    /// This is equivalent to:
    /// ```ignore
    /// let router = Router::new()
    ///     .route("/ping", get(handlers::ping).post(handlers::ping))
    ///     .route("/ping.view", get(handlers::ping).post(handlers::ping))
    ///     .route("/getLicense", get(handlers::get_license).post(handlers::get_license))
    ///     .route("/getLicense.view", get(handlers::get_license).post(handlers::get_license));
    /// ```
    #[must_use]
    fn subsonic_route<H, T>(self, path: &str, handler: H) -> Self
    where
        H: Handler<T, S> + Clone,
        T: 'static;
}

impl<S> SubsonicRouterExt<S> for Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    fn subsonic_route<H, T>(self, path: &str, handler: H) -> Self
    where
        H: Handler<T, S> + Clone,
        T: 'static,
    {
        let view_path = format!("{path}.view");
        self.route(path, get(handler.clone()).post(handler.clone()))
            .route(&view_path, get(handler.clone()).post(handler))
    }
}
