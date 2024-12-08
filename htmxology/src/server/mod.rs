use std::{future::Future, net::SocketAddr, pin::Pin, sync::Arc};

#[cfg(feature = "auto-reload")]
pub mod auto_reload;

/// The server information.
///
/// This information is made available in the Axum router state.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// The base URL of the server.
    pub base_url: http::Uri,
}

/// The options for the server.
#[derive(Debug, Clone, Default)]
pub struct ServerOptions {
    /// The base HTTP URL of the server.
    ///
    /// If the server is running behind a reverse proxy, this should be set to the base URL of the
    /// proxy.
    ///
    /// If no base URL is set, the server will attempt to determine the base URL from its own TCP
    /// listener address.
    ///
    /// If `HTMXOLOGY_BASE_URL` is set in the environment, it will be read and used as the base URL
    /// when calling `ServerOptions::from_env`.
    pub base_url: Option<http::Uri>,
}

/// An error that can occur when trying to get the server options from the environment.
#[derive(Debug, thiserror::Error)]
pub enum ServerOptionsFromEnvError {
    /// An environment variable was not unicode.
    #[error("environment variable {name} was not unicode")]
    NotUnicode {
        /// The name of the environment variable.
        name: &'static str,
    },

    /// An error occurred while trying to get the base URL from the environment.
    #[error("failed to parse the base URL from environment variable {name} (was `{url}`): {err}")]
    BaseUrl {
        /// The name of the environment variable.
        name: &'static str,

        /// The URL that was attempted to be parsed.
        url: String,

        /// The error that occurred.
        #[source]
        err: http::uri::InvalidUri,
    },
}

impl ServerOptions {
    /// The environment variable name for the base URL.
    pub const HTMXOLOGY_BASE_URL: &'static str = "HTMXOLOGY_BASE_URL";

    fn env_var(name: &'static str) -> Result<Option<String>, ServerOptionsFromEnvError> {
        match std::env::var(name) {
            Ok(value) => Ok(if value.is_empty() { None } else { Some(value) }),
            Err(std::env::VarError::NotPresent) => Ok(None),
            Err(std::env::VarError::NotUnicode(_)) => {
                Err(ServerOptionsFromEnvError::NotUnicode { name })
            }
        }
    }

    /// Get the server options from the environment.
    pub fn from_env() -> Result<Self, ServerOptionsFromEnvError> {
        tracing::info!("Reading HTMX SSR server options from the environment...");

        let base_url = Self::env_var(Self::HTMXOLOGY_BASE_URL)?
            .map(|url| {
                url.parse()
                    .map_err(|err| ServerOptionsFromEnvError::BaseUrl {
                        name: Self::HTMXOLOGY_BASE_URL,
                        url: url.clone(),
                        err,
                    })
            })
            .transpose()?;

        match &base_url {
            Some(base_url) => {
                tracing::info!(
                    "{} was set: using `{base_url}` as the base URL.",
                    Self::HTMXOLOGY_BASE_URL
                );
            }
            None => {
                tracing::warn!(
                    "{} was not set: base URL will be determined from the TCP listener address. This may not be what you want.",
                    Self::HTMXOLOGY_BASE_URL
                );
            }
        };

        Ok(Self { base_url })
    }
}

use crate::Route;

/// The server state type.
pub use super::State;

/// The Axum router type for the HTMX-SSR server.
pub type Router<Model> = axum::Router<State<Model>>;

/// The main struct for the HTMX-SSR framework.
///
/// Represents a running HTMX-SSR server.
pub struct Server<Model = ()> {
    /// The TCP listener that the server is using.
    listener: tokio::net::TcpListener,

    /// The Axum router that the server is using.
    router: Router<Model>,

    /// The graceful shutdown signal.
    graceful_shutdown: Option<Pin<Box<dyn Future<Output = ()> + Send>>>,

    /// The options for the server.
    options: ServerOptions,

    /// The user-defined model.
    model: Model,
}

/// An error that can occur when instantiating a new HTMX-SSR server with auto-reload features.
#[cfg(feature = "auto-reload")]
#[derive(Debug, thiserror::Error)]
pub enum NewWithAutoReloadError {
    /// An error occurred while trying to get a TCP listener.
    #[error("failed to get a TCP listener: {0}")]
    GetTcpListener(#[from] auto_reload::GetTcpListenerError),
}

/// An error that can occur when trying to serve the application.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    /// An error occurred while trying to serve the application.
    #[error("failed to serve the application: {0}")]
    Io(#[from] std::io::Error),

    /// An error occurred while trying to get the local address of the listener.
    #[error("failed to get the local address of the listener: {0}")]
    LocalAddr(std::io::Error),
}

impl<T> Server<T> {
    /// Get mutable access to the router.
    ///
    /// This is useful for adding routes to the server at a lower level.
    pub fn router(&mut self) -> &mut Router<T> {
        &mut self.router
    }

    /// Set the router on the server.
    pub fn with_router(mut self, router: Router<T>) -> Self {
        self.router = router;
        self
    }

    /// Get mutable access to the options.
    pub fn options(&mut self) -> &mut ServerOptions {
        &mut self.options
    }

    /// Set the options on the server.
    pub fn with_options(mut self, options: ServerOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the options on the server from the environment.
    pub fn with_options_from_env(mut self) -> Result<Self, ServerOptionsFromEnvError> {
        self.options = ServerOptions::from_env()?;

        Ok(self)
    }

    /// Set the graceful shutdown signal.
    pub fn with_graceful_shutdown(
        mut self,
        signal: impl Future<Output = ()> + Send + 'static,
    ) -> Self {
        self.graceful_shutdown = Some(Box::pin(signal));
        self
    }

    /// Set the graceful shutdown signal to `ctrl-c`.
    #[cfg(feature = "auto-reload")]
    pub fn with_ctrl_c_graceful_shutdown(self) -> Self {
        self.with_graceful_shutdown(async move {
            tracing::info!("Listening for `ctrl-c` signal for graceful shutdown...");

            if let Err(err) = tokio::signal::ctrl_c().await {
                tracing::error!("Failed to register for `ctrl-c` signal: {err}");
            }

            tracing::info!("Received `ctrl-c` signal, shutting down gracefully.");
        })
    }
}

impl<Model: Send + Sync + Clone + 'static> Server<Model> {
    /// Instantiate a new HTMX-SSR server.
    pub fn new(listener: tokio::net::TcpListener, model: Model) -> Self {
        let router = Default::default();
        let graceful_shutdown = None;
        let options = Default::default();

        Self {
            listener,
            router,
            graceful_shutdown,
            options,
            model,
        }
    }

    /// Instantiate a new HTMX-SSR server with all the auto-reload features enabled.
    ///
    /// Attempts to get a TCP listener from the environment if run through `listenfd`, falling
    /// back to binding to a local address if that fails.
    ///
    /// Also sets the graceful shutdown signal to `ctrl-c`.
    #[cfg(feature = "auto-reload")]
    pub async fn new_with_auto_reload(
        addr: impl tokio::net::ToSocketAddrs,
        model: Model,
    ) -> Result<Self, NewWithAutoReloadError> {
        let listener = auto_reload::get_or_bind_tcp_listener(addr).await?;

        Ok(Self::new(listener, model).with_ctrl_c_graceful_shutdown())
    }
    /// Serve the application.
    pub async fn serve(self) -> Result<(), ServeError> {
        let local_addr = self.listener.local_addr().map_err(ServeError::LocalAddr)?;

        tracing::info!("HTMX SSR server listening on TCP/{local_addr}.");

        let base_url = match self.options.base_url {
            Some(base_url) => base_url,
            None => Self::guess_base_url(local_addr),
        };

        let server = Arc::new(ServerInfo { base_url });

        let state = super::State {
            server,
            model: self.model,
        };

        tracing::info!(
            "Now serving HTMX SSR server at `{}`...",
            state.server.base_url
        );

        let router = self.router.with_state(state);
        let serve = axum::serve(self.listener, router);

        match self.graceful_shutdown {
            Some(signal) => serve.with_graceful_shutdown(signal).await,
            None => serve.await,
        }
        .map_err(Into::into)
    }

    /// Guess the base URL from the local address.
    fn guess_base_url(local_addr: SocketAddr) -> http::Uri {
        tracing::info!("No base URL set, guessing from local address `{local_addr}`...");

        if local_addr.ip().is_unspecified() {
            // If the local address is unspecified, we have to enumerate the network
            // interfaces and take an address from one of them.
            tracing::warn!("Local address is unspecified, guessing from network interfaces... This is likely not what you want.");

            let localhost_base_url = format!("http://localhost:{}", local_addr.port())
                .parse()
                .expect("hardcoded URL is valid");

            #[cfg(feature = "interfaces")]
            match netdev::get_default_interface() {
                Ok(interface) => {
                    tracing::info!(
                        "Using default network interface `{}` for the base URL.",
                        interface.friendly_name.unwrap_or(interface.name)
                    );

                    match interface
                        .ipv4
                        .into_iter()
                        .map(|ip_v4| ip_v4.addr().to_string())
                        .chain(
                            interface
                                .ipv6
                                .into_iter()
                                .map(|ip_v6| ip_v6.addr().to_string()),
                        )
                        .next()
                    {
                        Some(ip) => {
                            return format!("http://{}:{}", ip, local_addr.port())
                                .parse()
                                .expect("hardcoded URL is valid");
                        }
                        None => {
                            tracing::error!(
                                "No IP address found for the default network interface."
                            );
                        }
                    };
                }
                Err(err) => {
                    tracing::error!("Failed to determine default network interface: {err}");
                }
            }

            localhost_base_url
        } else {
            format!("http://{}:{}", local_addr.ip(), local_addr.port())
                .parse()
                .expect("hardcoded URL is valid")
        }
    }

    /// Register a controller with the server.
    ///
    /// This erases all registered routes and replaces them with the routes defined by the
    /// controller.
    pub fn with_controller<Controller: super::Controller<Model = Model>>(mut self) -> Self {
        self.router = Controller::Route::make_router::<Controller>();
        self
    }
}
