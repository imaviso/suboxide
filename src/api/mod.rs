//! Subsonic API module.

pub mod auth;
pub mod error;
pub mod handlers;
pub mod response;
pub mod router;

#[doc(inline)]
pub use auth::{AuthState, DatabaseAuthState, SubsonicAuth};
#[doc(inline)]
pub use error::ApiError;
#[doc(inline)]
pub use response::{Format, ok_empty, ok_license};
#[doc(inline)]
pub use router::SubsonicRouterExt;
