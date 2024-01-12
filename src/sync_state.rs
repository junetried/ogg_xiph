use core::mem::MaybeUninit;
use std::{
	num::NonZeroUsize,
	os::raw::c_long
};
use ogg_next_sys::*;
use crate::Page;

/// The `SyncState` is responsible for decoding and syncing [Pages](Page).
/// 
/// ## Usage
/// 
/// Use [submit_bytes](SyncState::submit_bytes) to write bytes of a
/// physical Ogg stream, and get back [Pages](Page) from it if any
/// could be returned:
/// 
/// ```rust
/// # use ogg_xiph::SyncState;
/// # let bytes = vec![0; 28];
/// let mut sync_state = SyncState::new().expect("SyncState should initialize");
/// // `submit_bytes` takes a &[u8], replace this with your input bytes
/// let pages = sync_state.submit_bytes(&bytes);
/// 
/// match pages {
/// 	Ok(None) => println!("No pages could be assembled from bytes!"),
/// 	Ok(Some(pages)) => {
/// 		println!("Pages were assembled from bytes!");
/// 
/// 		// Do something with the pages...
/// 		for page in pages {
/// 			println!("Version: {}", page.version());
/// 			println!("Begins a logical stream: {}", page.begins_logical_stream());
/// 			println!("Ends a logical stream: {}", page.ends_logical_stream());
/// 			println!("Granule position: {}", page.absgp());
/// 		}
/// # 		panic!("zeroed bytes produced a page")
/// 	},
/// 	Err(error) => panic!("SyncState returned error from bytes! error: {}", error)
/// }
/// ```
pub struct SyncState {
		sync_state: ogg_sync_state
}

impl SyncState {
		/// Return an initialized `SyncState`.
		pub fn new() -> Result<Self, ()> {
			let mut sync_state: MaybeUninit<ogg_sync_state> = MaybeUninit::uninit();
			let code = unsafe {
				ogg_sync_init(sync_state.as_mut_ptr())
			};
			if code == 0 {
				Ok(Self { sync_state: unsafe { sync_state.assume_init() } })
			} else { Err(()) }
		}

		/// Reset this `SyncState` to a new state.
		pub fn reset(&mut self) {
			let code;
		unsafe {
				code = ogg_sync_reset(&mut self.sync_state as *mut ogg_sync_state)
			};
			assert!(code == 0)
		}

		/// Check whether this `SyncState` is currently in sync.
		pub fn is_synced(&self) -> bool {
			// Unsynced if the `unsynced` field is 'nonzero'
			self.sync_state.unsynced == 0
		}

		/// Provide a buffer for writing to the [ogg_sync_state].
		fn buffer(&mut self, size: NonZeroUsize) -> &mut [u8] {
			let buffer = unsafe {
				ogg_sync_buffer(&mut self.sync_state as *mut ogg_sync_state, usize::from(size) as c_long)
			}.cast::<u8>();

			if buffer.is_null() {
				panic!("ogg_sync_buffer returned null pointer")
			}

			unsafe { std::slice::from_raw_parts_mut(buffer, usize::from(size)) }
		}

		/// Tells the [ogg_sync_state] how many bytes have been written to the buffer.
		fn wrote(&mut self, size: std::num::NonZeroUsize) {
			let code = unsafe {
				ogg_sync_wrote(&mut self.sync_state as *mut ogg_sync_state, usize::from(size) as c_long)
			};

			if code == -1 { panic!("ogg_sync_wrote returned an error: writing buffer of size {} overflows into ogg_sync_state", size) }
			assert!(code == 0)
		}

		/// Write bytes to the `SyncState`.
		fn write(&mut self, bytes: &[u8]) {
			let size = NonZeroUsize::try_from(bytes.len())
				.expect("non zero usize");
			let buffer = self.buffer(size);

			assert!(buffer.len() >= bytes.len());

			for (index, byte) in bytes.iter().enumerate() {
				buffer[index] = *byte
			}

			self.wrote(size);
		}

		/// Write an [ogg_page].
		fn page_out(&mut self, page: *mut ogg_page) -> Result<(), PageWriteError> {
			let code = unsafe {
				ogg_sync_pageout(&mut self.sync_state as *mut ogg_sync_state, page)
			};

			match code {
				 -1 => Err(PageWriteError::OutOfSync),
				 0 => Err(PageWriteError::InternalError),
				 1 => Ok(()),
				 unexpected => panic!("ogg_sync_pageout should only return -1, 0, or 1, but returned {}", unexpected)
			}
		}

		/// Write bytes to the `SyncState` and return all [Pages](Page),
		/// if any, that were completed from the input bytes.
		pub fn submit_bytes(&mut self, bytes: &[u8]) -> Result<Option<Vec<Page>>, PageWriteError> {
			self.write(bytes);
			let mut page: MaybeUninit<ogg_page> = MaybeUninit::uninit();
			let mut collected = vec![];

			if self.page_out(page.as_mut_ptr()).is_err() { return Ok(None) }
			let mut page = unsafe { page.assume_init() };
			collected.push(
				unsafe {
					match Page::try_from(page) {
						Err(_) => return Err(PageWriteError::InvalidPage),
						Ok(page) => page.clone()
					}
				}
			);

			loop {
				if self.page_out(&mut page).is_err() { break }
				collected.push(
					unsafe {
						match Page::try_from(page) {
							Err(_) => return Err(PageWriteError::InvalidPage),
							Ok(page) => page.clone()
						}
					}
				);
			}
			Ok(Some(collected))
		}

		/// Synchronizes to the next Page.
		pub fn page_seek(&mut self, _: &mut Page) {
			todo!()
		}
}

impl Drop for SyncState {
	fn drop(&mut self) {
		let code;
		unsafe {
			code = ogg_sync_clear(&mut self.sync_state as *mut ogg_sync_state)
		};
		// 0 is always returned
		assert!(code == 0)
	}
}

/// An error that can happen while writing a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageWriteError {
	OutOfSync,
	InternalError,
	InvalidPage
}

impl std::fmt::Display for PageWriteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Self::OutOfSync => write!(f, "stream has not captured sync, bytes were skipped"),
			Self::InternalError => write!(f, "not enough data has been submitted to complete a page or an internal error occurred"),
			Self::InvalidPage => write!(f, "ogg returned an invalid page")
		}
    }
}
