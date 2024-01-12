use std::os::raw::c_long;
use ogg_next_sys::*;

pub const HEADER_VERSION: usize = 4;
pub const HEADER_TYPE: usize = 5;
pub const HEADER_GRANULE_POSITION: usize = 6;
pub const HEADER_PAGE_SERIAL_NUMBER: usize = 14;
pub const HEADER_SEQUENCE_NUMBER: usize = 18;
pub const HEADER_CHECKSUM: usize = 22;
// pub const HEADER_SEGMENTS: usize = 26;
pub const HEADER_SIZE_MIN: usize = 28;

/// A privately owned version of the [ogg_page] struct.
#[derive(Clone)]
pub(crate) struct PrivatePage {
	pub header: Vec<u8>,
	pub body: Vec<u8>
}

impl PrivatePage {
	/// Create a new `PrivatePage`.
	pub fn new() -> Self {
		Self {
			header: Vec::with_capacity(1),
			body: Vec::with_capacity(1)
		}
	}

	/// Get an [ogg_page] from this `Page`.
	pub fn ogg_page(&mut self) -> ogg_page {
		ogg_page {
			header: self.header.as_mut_ptr(),
			header_len: self.header.len().try_into().expect("usize as c_long"),
			body: self.body.as_mut_ptr(),
			body_len: self.body.len().try_into().expect("usize as c_long")
		}
	}
}

pub struct Page {
	/// The underlying page struct.
	/// 
	/// This might contain pointers which point to data owned
	/// by ogg.
	pub(crate) page: ogg_page,
	pub(crate) owned: Option<PrivatePage>
}

impl Page {
	/// Create a new `Page`.
	pub fn new() -> Self {
		let mut owned = PrivatePage::new();

		Self { page: owned.ogg_page(), owned: Some(owned) }
	}

	/// Try to create a [Page] from an [ogg_page].
	/// 
	/// Will fail if `body_len` or `header_len` can't be read as
	/// [usize].
	/// 
	/// This function is `unsafe` because the underlying
	/// `ogg_page` contains raw pointers.
	pub unsafe fn try_from(page: ogg_page) -> Result<Self, InvalidPage> {
		if let Err(usize_error) = usize::try_from(page.body_len) { return Err(InvalidPage::BadPointer(usize_error)) };
		if let Err(usize_error) = usize::try_from(page.header_len) { return Err(InvalidPage::BadPointer(usize_error)) };

		let page = Self { page, owned: None };
		if let Err(header_error) = validate_header(page.header()) { return Err(InvalidPage::InvalidHeader(header_error)) };
		
		Ok(page)
	}

	unsafe fn length_from_ogg_page(&mut self) {
		match &mut self.owned {
			None => {},
			Some(owned) => {
				let header_len = self.page.header_len.try_into().expect("c_long as usize");
				let body_len = self.page.body_len.try_into().expect("c_long as usize");
				assert!(owned.header.capacity() >= header_len);
				owned.header.set_len(header_len);
				assert!(owned.body.capacity() >= body_len);
				owned.body.set_len(body_len)
			}
		}
	}

	/// Set the data of this `Page`.
	pub fn set_data(&mut self, data: Vec<u8>) {
		match &mut self.owned {
			None => panic!("mutating a Page which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned) => {
				if data.capacity() == 0 {
					owned.body = Vec::with_capacity(1)
				} else {
					owned.body = data;
				}
			}
		}
	}

	/// Set the data of this `Page`.
	pub fn set_header(&mut self, header: Vec<u8>) -> Result<(), InvalidPageHeader> {
		match &mut self.owned {
			None => panic!("mutating a Page which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned) => {
				validate_header(&header)?;
				if header.capacity() == 0 {
					owned.header = Vec::with_capacity(1)
				} else {
					owned.header = header;
				}
			}
		}

		Ok(())
	}

	/// Get an [ogg_page] from this `Page`.
	pub fn ogg_page(&mut self) -> &mut ogg_page {
		if let Some(owned) = &mut self.owned {
			// Otherwise put pointers into the struct
			self.page = owned.ogg_page();

			&mut self.page
		} else {
			// Pages from a `Stream` will be returned here
			&mut self.page
		}
	}

	/// Return a reference to the data for this `Page`.
	pub fn data(&self) -> &[u8] {
		match &self.owned {
			None => unsafe { core::slice::from_raw_parts(self.page.body, self.page.body_len as usize) },
			Some(owned_page) => &owned_page.body
		}
	}

	/// Return a mutable reference to the data for this `Page`.
	pub fn data_mut(&mut self) -> &mut [u8] {
		match &mut self.owned {
			None => panic!("mutating a Page which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned_page) => &mut owned_page.body
		}
		
	}

	/// Return a reference to the raw header for this `Page`.
	pub fn header(&self) -> &[u8] {
		match &self.owned {
			None => unsafe { core::slice::from_raw_parts(self.page.header, self.page.header_len as usize) },
			Some(owned_page) => &owned_page.header
		}
	}

	/// Return a mutable reference to the raw header for this `Page`.
	pub fn header_mut(&mut self) -> &mut [u8] {
		match &mut self.owned {
			None => panic!("mutating a Page which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned_page) => &mut owned_page.header
		}
		
	}

	/// Returns the `Page` version.
	/// 
	/// In the current version of Ogg, this should always be zero.
	/// Any other value means there is an error in the page.
	pub fn version(&self) -> u8 {
		self.header()[HEADER_VERSION]
	}

	/// Returns the `Page` header type.
	/// 
	/// This signals the following combined values:
	/// - **1**: Page contains a packet which continues from a previous page.
	/// - **2**: Page is the first page of its stream.
	/// - **4**: Page is the last page of its stream.
	pub fn header_type(&self) -> u8 {
		self.header()[HEADER_TYPE]
	}

	/// Check whether this `Page` contains packet data that continues
	/// from the last `Page`.
	pub fn continues_packet(&self) -> bool {
		[1, 3, 7].contains(&self.header_type())
	}

	/// Return the number of packets that completed on this `Page`.
	/// This *includes* packets that begin on a previous `Page`.
	/// 
	/// This is not necessarily a non-zero value. If a packet
	/// happens to begin on a previous page and span to a future
	/// page, in the case of a packet that spans three or more
	/// pages, the return value of this method would be 0.
	pub fn finished_packets(&mut self) -> u8 {
		unsafe { ogg_page_packets(self.ogg_page()) as u8 }
	}

	/// Check whether this page begins a logical stream.
	pub fn begins_logical_stream(&self) -> bool {
		[2, 6, 7].contains(&self.header_type())
	}

	/// Check whether this `Page` ends a logical [Stream](crate::Stream).
	pub fn ends_logical_stream(&self) -> bool {
		[4, 6, 7].contains(&self.header_type())
	}

	/// Return the absolute granule position of the packet data
	/// at the end of this `Page`.
	pub fn absgp(&self) -> u64 {
		u64::from_le_bytes(self.header()[HEADER_GRANULE_POSITION..HEADER_GRANULE_POSITION + 8].try_into().unwrap())
	}

	/// Return the serial number of the logical stream that this
	/// `Page` is associated with.
	pub fn stream_serial(&self) -> i32 {
		i32::from_le_bytes(self.header()[HEADER_PAGE_SERIAL_NUMBER..HEADER_PAGE_SERIAL_NUMBER + 4].try_into().unwrap())
	}

	/// Return the sequential number for this `Page`.
	/// 
	/// This can be used for ordering pages or detecting pages
	/// that have been lost.
	pub fn index(&self) -> u32 {
		u32::from_le_bytes(self.header()[HEADER_SEQUENCE_NUMBER..HEADER_SEQUENCE_NUMBER + 4].try_into().unwrap())
	}

	/// Return the CRC checksum of this `Page`.
	/// 
	/// This can be used for ordering pages or detecting pages
	/// that have been lost.
	pub fn crc_checksum(&self) -> u32 {
		u32::from_le_bytes(self.header()[HEADER_CHECKSUM..HEADER_CHECKSUM + 4].try_into().unwrap())
	}

	/// Return the CRC checksum of this `Page`.
	/// 
	/// This can be used for ordering pages or detecting pages
	/// that have been lost.
	pub fn set_crc_checksum(&mut self) {
		unsafe { ogg_page_checksum_set(self.ogg_page()) }
	}
}

impl Clone for Page {
    fn clone(&self) -> Self {
		if let Some(owned) = &self.owned {
			let mut owned = owned.clone();
			Self { page: owned.ogg_page(), owned: Some(owned) }
		} else {
			let mut owned = PrivatePage {
				header: self.header().to_vec(),
				body: self.data().to_vec()
			};
			let page = owned.ogg_page();
	        Self { page, owned: Some(owned) }
		}
    }
}

pub fn validate_header(header: &[u8]) -> Result<(), InvalidPageHeader> {
	if header.len() < HEADER_SIZE_MIN { return Err(InvalidPageHeader::TooShort) }
	if header[0..4] != [79, 103, 103, 83] { return Err(InvalidPageHeader::NoMagicString) }
	if header[HEADER_VERSION] != 0 { return Err(InvalidPageHeader::BadVersion(header[HEADER_VERSION])) };
	Ok(())
}

/// Error validating the [ogg_page].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidPage {
	/// The pointer returned couldn't be read as usize.
	BadPointer (<usize as TryFrom<c_long>>::Error),
	InvalidHeader (InvalidPageHeader)
}

impl std::fmt::Display for InvalidPage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Self::BadPointer (usize_error) => write!(f, "ogg returned an invalid pointer: {}", usize_error),
			Self::InvalidHeader (header_error) => write!(f, "ogg returned invalid header: {}", header_error)
		}
    }
}

/// Error validating the page header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidPageHeader {
	/// The first four bytes were not 'OggS'.
	NoMagicString,
	/// The header version was wrong.
	BadVersion (u8),
	/// The header was too short.
	TooShort
}

impl std::fmt::Display for InvalidPageHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Self::NoMagicString => write!(f, "header has an invalid magic string (should be 'OggS')"),
			Self::BadVersion(v) => write!(f, "version number is {} (should be 0)", v),
			Self::TooShort => write!(f, "page header is too short")
		}
    }
}
