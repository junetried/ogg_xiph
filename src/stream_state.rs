use std::{
	mem::MaybeUninit,
	num::NonZeroUsize,
	os::raw::c_int
};
use ogg_next_sys::*;

use crate::{ Packet, Page, InternalError };

/// This struct is responsible for managing the current encode
/// and decode state of a logical stream.
/// 
/// For decoding Ogg streams, see the relevant methods
/// [page_in](Stream::page_in) and [packet_out](Stream::packet_out).
/// 
/// For encoding Ogg streams, see the relevant methods
/// [packet_in](Stream::packet_in) and [page_out](Stream::page_out).
pub struct Stream {
	stream_state: ogg_stream_state,
	/// Keep track of whetther or not the stream has at least
	/// one page submitted to process.
	has_pages: bool,
	/// Page data. This points to data **owned by ogg** and we
	/// should be careful with it.
	page_buffer: Option<Page>,
	/// Packet data. This points to data **owned by ogg** and we
	/// should be careful with it.
	packet_buffer: Option<Packet>
}

impl Stream {
	/// Return an initialized `Stream`.
	pub fn new(serial: i32) -> Result<Self, ()> {
		let mut stream_state: MaybeUninit<ogg_stream_state> = MaybeUninit::uninit();
		let code = unsafe {
			ogg_stream_init(stream_state.as_mut_ptr(), serial as c_int)
		};
		if code == 0 {
			Ok(Self { stream_state: unsafe { stream_state.assume_init() }, has_pages: false, page_buffer: None, packet_buffer: None })
		} else { Err(()) }
	}

	/// Reset this `Stream` back to an initial state.
	pub fn reset(&mut self) -> Result<(), ()> {
		self.page_buffer = None;
		self.packet_buffer = None;
		match unsafe {
			ogg_stream_reset(&mut self.stream_state as *mut ogg_stream_state)
		} {
			0 => { self.has_pages = false; Ok(()) },
			_ => Err(())
		}
	}

	/// Check if the `Stream` has ended, possibly due to error.
	pub fn end_of_stream(&mut self) -> bool {
		self.page_buffer = None;
		self.packet_buffer = None;
		match unsafe {
			ogg_stream_eos(&mut self.stream_state as *mut ogg_stream_state)
		} {
			0 => false,
			1 => true,
			unexpected => panic!("ogg_stream_eos should always return 0 or 1 but returned {}", unexpected)
		}
	}

	/// Add a `Page` to the `Stream`.
	pub fn page_in(&mut self, page: &mut Page) -> Result<(), PageInError> {
		self.page_buffer = None;
		self.packet_buffer = None;
		if page.stream_serial() != self.stream_state.serialno as c_int {
			return Err(PageInError::WrongSerial((self.stream_state.serialno as c_int, page.stream_serial())))
		}

		unsafe {
			let page = page.ogg_page();
			match ogg_stream_pagein(&mut self.stream_state as *mut ogg_stream_state, page as *mut ogg_page) {
				-1 => Err(PageInError::InternalError("ogg_stream_pagein".to_string())),
				0 => { self.has_pages = true; Ok(()) },
				unexpected => panic!("ogg_stream_pagein should always return 0 or -1 but returned {}", unexpected)
			}
		}
	}

	/// Export a packet from the `Stream`.
	/// 
	/// This should be run *after* submitting at least one `Page` to the stream.
	pub fn packet_out(&mut self) -> Result<&Packet, PacketOutError> {
		self.page_buffer = None;
		self.packet_buffer = None;
		if !self.has_pages {
			return Err(PacketOutError::NoPages)
		}

		let mut packet: MaybeUninit<ogg_packet> = MaybeUninit::uninit();

		unsafe {
			match ogg_stream_packetout(&mut self.stream_state as *mut ogg_stream_state, packet.as_mut_ptr()) {
				-1 => Err(PacketOutError::OutOfSync),
				0 => Err(PacketOutError::InternalError),
				1 => {
					self.packet_buffer = Some(
						Packet::try_from(packet.assume_init())
							.expect("packet returned from ogg_stream_packetout should be valid")
					);
					Ok(self.packet_buffer.as_ref().unwrap())
				},
				unexpected => panic!("ogg_stream_packetout should always return 0 or -1 but returned {}", unexpected)
			}
		}
	}

	/// Peek the next `Packet` in the `Stream` without advancing decoding.
	/// 
	/// This should be run *after* submitting at least one `Page` to the stream.
	pub fn packet_peek(&mut self) -> Result<&Packet, PacketOutError> {
		// Packet data is owned by ogg
		// https://xiph.org/ogg/doc/libogg/ogg_stream_packetout.html
		self.page_buffer = None;
		self.packet_buffer = None;
		if !self.has_pages {
			return Err(PacketOutError::NoPages)
		}

		let mut packet: MaybeUninit<ogg_packet> = MaybeUninit::uninit();

		unsafe {
			match ogg_stream_packetpeek(&mut self.stream_state as *mut ogg_stream_state, packet.as_mut_ptr()) {
				-1 => Err(PacketOutError::OutOfSync),
				0 => Err(PacketOutError::InternalError),
				1 => {
					self.packet_buffer = Some(
						Packet::try_from(packet.assume_init())
							.expect("packet returned from ogg_stream_packetpeek should be valid")
					);
					Ok(self.packet_buffer.as_ref().unwrap())
				},
				unexpected => panic!("ogg_stream_packetpeek should always return 0 or -1 but returned {}", unexpected)
			}
		}
	}

	/// Add a `Packet` to the `Stream`.
	pub fn packet_in(&mut self, packet: &mut Packet) -> Result<(), InternalError> {
		self.page_buffer = None;
		self.packet_buffer = None;
		unsafe {
			match ogg_stream_packetin(&mut self.stream_state as *mut ogg_stream_state, &mut packet.packet as *mut ogg_packet) {
				-1 => Err(InternalError("ogg_stream_packetin".to_string())),
				0 => { self.has_pages = true; Ok(()) },
				unexpected => panic!("ogg_stream_packetin should always return 0 or -1 but returned {}", unexpected)
			}
		}
	}

	/// Export a `Page` from the `Stream`.
	pub fn page_out(&mut self) -> Result<&Page, PacketOutError> {
		// Page is owned by ogg
		// https://xiph.org/ogg/doc/libogg/ogg_stream_pageout.html
		self.page_buffer = None;
		self.packet_buffer = None;
		let mut page: MaybeUninit<ogg_page> = MaybeUninit::uninit();

		unsafe {
			match ogg_stream_pageout(&mut self.stream_state as *mut ogg_stream_state, page.as_mut_ptr()) {
				0 => Err(PacketOutError::InternalError),
				_ => {
					self.page_buffer = Some(Page::try_from(page.assume_init()).expect("packet returned from ogg_stream_pageout should be valid"));
					Ok(self.page_buffer.as_ref().unwrap())
				}
			}
		}
	}

	/// Export a `Page` from the `Stream` with at most the given size in bytes.
	pub fn page_out_with_max_size(&mut self, size: NonZeroUsize) -> Result<&Page, PacketOutError> {
		// Page is owned by us
		// https://xiph.org/ogg/doc/libogg/ogg_stream_pageout_fill.html
		self.page_buffer = None;
		self.packet_buffer = None;
		let mut page: MaybeUninit<ogg_page> = MaybeUninit::uninit();

		unsafe {
			match ogg_stream_pageout_fill(&mut self.stream_state as *mut ogg_stream_state, page.as_mut_ptr(), usize::from(size) as c_int) {
				0 => Err(PacketOutError::InternalError),
				_ => {
					self.page_buffer = Some(Page::try_from(page.assume_init()).expect("packet returned from ogg_stream_pageout should be valid"));
					Ok(self.page_buffer.as_ref().unwrap())
				}
			}
		}
	}

	/// Flush remaining packets in the `Stream` into a `Page`.
	/// 
	/// This will force create a page, even if it is undersized.
	/// You can use this if you want a page from the middle
	/// of the stream for some reason, but you probably don't.
	/// 
	/// If you just want to get the next page from the stream,
	/// see [page_out](Stream::page_out) or [page_out_with_max_size](Stream::page_out_with_max_size).
	/// 
	/// This can be used to verify that the stream has no
	/// more packets to flush.
	pub fn page_flush(&mut self) -> Result<&Page, PacketOutError> {
		// Page is owned by us
		// https://xiph.org/ogg/doc/libogg/ogg_stream_flush.html
		self.page_buffer = None;
		self.packet_buffer = None;
		let mut page: MaybeUninit<ogg_page> = MaybeUninit::uninit();

		unsafe {
			match ogg_stream_flush(&mut self.stream_state as *mut ogg_stream_state, page.as_mut_ptr()) {
				0 => Err(PacketOutError::InternalError),
				_ => {
					self.page_buffer = Some(Page::try_from(page.assume_init()).expect("packet returned from ogg_stream_pageout should be valid"));
					Ok(self.page_buffer.as_ref().unwrap())
				}
			}
		}
	}

	/// Flush remaining packets in the `Stream` into a `Page`
	/// with at most the given size in bytes.
	/// 
	/// This will force create a page, even if it is undersized.
	/// You can use this if you want a page from the middle
	/// of the stream for some reason, but you probably don't.
	/// 
	/// If you just want to get the next page from the stream,
	/// see [page_out](Stream::page_out) or [page_out_with_max_size](Stream::page_out_with_max_size).
	/// 
	/// This can be used to verify that the stream has no
	/// more packets to flush.
	pub fn page_flush_with_max_size(&mut self, size: NonZeroUsize) -> Result<&Page, PacketOutError> {
		// Page is owned by us
		// https://xiph.org/ogg/doc/libogg/ogg_stream_flush_fill.html
		self.page_buffer = None;
		self.packet_buffer = None;
		let mut page: MaybeUninit<ogg_page> = MaybeUninit::uninit();

		unsafe {
			match ogg_stream_flush_fill(&mut self.stream_state as *mut ogg_stream_state, page.as_mut_ptr(), usize::from(size) as c_int) {
				0 => Err(PacketOutError::InternalError),
				_ => {
					self.page_buffer = Some(Page::try_from(page.assume_init()).expect("packet returned from ogg_stream_pageout should be valid"));
					Ok(self.page_buffer.as_ref().unwrap())
				}
			}
		}
	}
}

impl Drop for Stream {
	fn drop(&mut self) {
		let code = unsafe {
			ogg_stream_clear(&mut self.stream_state as *mut ogg_stream_state)
		};
		// 0 is always returned
		assert!(code == 0)
	}
}

/// An error returned while adding a page to the stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageInError {
	/// Stream was given a packet with a
	/// mis-matched serial number.
	/// 
	/// `(expected serial, actual serial)`
	WrongSerial ((c_int, i32)),
	/// An internal error occurred in Ogg.
	InternalError (String)
}

impl std::fmt::Display for PageInError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Self::WrongSerial((expected_serial, actual_serial)) => write!(f, "serial number {} does not match this stream serial {}", actual_serial, expected_serial),
			Self::InternalError(function) => InternalError::fmt_str(f, function)
		}
    }
}

/// An error returned while adding a page to the stream.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PacketOutError {
	OutOfSync,
	NoPages,
	InternalError,
}

impl std::fmt::Display for PacketOutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Self::OutOfSync => write!(f, "stream fell out of sync, input might be incomplete"),
			Self::NoPages => write!(f, "no pages have been submitted to the stream yet"),
			Self::InternalError => write!(f, "there is not enough data to complete a packet or an internal error occurred")
		}
    }
}
