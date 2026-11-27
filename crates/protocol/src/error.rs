// Copyright © Fangdun Tsai <fundon@pindash.io>
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! The protocol crate's unified error type.

use snafu::Snafu;

/// The protocol crate's unified error.
#[derive(Debug, Snafu)]
pub enum Error {
    /// A catch-all error with a message and an optional source.
    #[snafu(whatever, display("{message}"))]
    GenericError {
        /// The error message.
        message: String,
        /// The underlying error, when one was wrapped.
        #[snafu(source(from(Box<dyn core::error::Error + Sync + Send>, Some)))]
        source: Option<Box<dyn core::error::Error + Sync + Send>>,
    },
    /// A shared mutex was poisoned by a panicking thread.
    #[snafu(display("mutex poisoned"))]
    Poisoned,
    /// An I/O error.
    #[snafu(transparent)]
    Io {
        /// The underlying I/O error.
        source: std::io::Error,
    },
}

/// The protocol crate's `Result` alias.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl<'a, T: ?Sized> From<std::sync::PoisonError<std::sync::MutexGuard<'a, T>>>
    for Error
{
    fn from(_: std::sync::PoisonError<std::sync::MutexGuard<'a, T>>) -> Self {
        Self::Poisoned
    }
}
