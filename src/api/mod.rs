//! Subsonic API module.

pub mod auth;
pub mod error;
pub mod handlers;
pub mod response;
pub mod router;
pub mod services;

#[doc(inline)]
pub use auth::SubsonicAuth;
#[doc(inline)]
pub use error::ApiError;
#[doc(inline)]
pub use response::{Format, SubsonicResponse};
#[doc(inline)]
pub use router::SubsonicRouterExt;
