//! # ogg_xiph
//! 
//! This crate provides an interface to the
//! [libogg library](https://xiph.org/ogg/).
//! 
//! This crate does *not* provide any function for decoding the
//! data in an Ogg stream. After you get packets, you'll need
//! to use another library for decoding them.
//! 
//! ## Usage
//! 
//! The basic structure you need to know looks like this:
//! 
//! ```text
//!             Ogg, the physical file or stream
//!                             V
//! Ogg pages, including metadata for assembling logical streams
//!                             V
//!          Ogg packets, the actual underlying data
//! ```
//! 
//! You can start with a [SyncState]:
//! 
//! ```rust
//! # use ogg_xiph::SyncState;
//! // The sync state may fail to initialize, so we should match that
//! let mut sync_state = match SyncState::new() {
//! 	Err(()) => panic!("initializing sync state failed!"),
//! 	Ok(sync_state) => sync_state
//! };
//! // And now we have a SyncState!
//! ```
//! 
//! Then submit bytes to the [SyncState] to get [Pages](Page)
//! out of it:
//! 
//! ```rust
//! # use ogg_xiph::{ SyncState, Page };
//! # let mut sync_state = SyncState::new().unwrap();
//! # let bytes = vec![0; 28];
//! // `&bytes` is a `&[u8]`.
//! let pages: Option<Vec<Page>> = match sync_state.submit_bytes(&bytes) {
//! 	// Successfully gathered page(s)
//! 	Ok(Some(pages)) => Some(pages),
//! 	// Not enough data was available to build a page
//! 	// This might also indicate an internal error
//! 	Ok(None) => None,
//! 	// An error occurred
//! 	Err(page_write_error) => panic!("ogg returned an error: {}", page_write_error)
//! };
//! ```
//! 
//! See the tests module for more examples.

// Forget you, Clippy.
#![allow(clippy::tabs_in_doc_comments)]

mod packet;
mod page;
mod stream_state;
mod sync_state;

pub use packet::{ Packet, PacketInitError };
pub use page::{ Page, InvalidPage, InvalidPageHeader };
pub use stream_state::{ Stream, PageInError, PacketOutError };
pub use sync_state::{ SyncState, PageWriteError };

#[cfg(test)]
mod tests;

/// An internal error in ogg.
/// 
/// `String` is the ogg function this occurred in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InternalError (pub String);

impl std::fmt::Display for InternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Self::fmt_str(f, &self.0)
    }
}

impl InternalError {
	fn fmt_str(f: &mut std::fmt::Formatter<'_>, function: &str) -> std::fmt::Result {
		write!(f, "an internal error occurred while running {}", function)
	}
}
