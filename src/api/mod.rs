//! Subsonic API module.

pub mod auth;
pub mod error;
pub mod handlers;
pub mod response;
pub mod router;

#[doc(inline)]
pub use auth::{AuthState, AuthStateHandle, DatabaseAuthState, SubsonicAuth};
#[doc(inline)]
pub use error::ApiError;
#[doc(inline)]
pub use response::{Format, SubsonicResponse};
#[doc(inline)]
pub use router::SubsonicRouterExt;
