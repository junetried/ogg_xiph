use ogg_next_sys::*;
use std::os::raw::c_long;

/// A privately owned version of the [ogg_packet] struct.
#[derive(Clone)]
pub(crate) struct PrivatePacket {
	pub data: Vec<u8>,
	pub beginning_of_stream: bool,
	pub end_of_stream: bool,
	pub absgp: u64,
	pub index: u32
}

impl PrivatePacket {
	/// Create a new `PrivatePacket`.
	pub fn new() -> Self {
		Self {
			data: Vec::with_capacity(1),
			beginning_of_stream: false,
			end_of_stream: false,
			absgp: 0,
			index: 0
		}
	}

	/// Get an [ogg_packet] from this `Packet`.
	pub fn ogg_packet(&mut self) -> ogg_packet {
		ogg_packet {
			packet: self.data.as_mut_ptr(),
			bytes: self.data.len().try_into().expect("usize as c_long"),
			b_o_s: if self.beginning_of_stream { 1 } else { 0 },
			e_o_s: if self.end_of_stream { 1 } else { 0 },
			granulepos: self.absgp as i64,
			packetno: self.index as i64
		}
	}
}

pub struct Packet {
	pub(crate) packet: ogg_packet,
	pub(crate) owned: Option<PrivatePacket>
}

impl Packet {
	/// Create a new `Page`.
	pub fn new() -> Self {
		let mut owned = PrivatePacket::new();

		Self { packet: owned.ogg_packet(), owned: Some(owned) }
	}

	/// Try to create a [Packet] from an [ogg_packet].
	/// 
	/// Will fail if `bytes` can't be read as `usize` or
	/// if `b_o_s` or `e_o_s` are any value other than
	/// `0` or `1`.
	/// 
	/// This function is `unsafe` because the underlying
	/// `ogg_packet` contains raw pointers.
	pub unsafe fn try_from(packet: ogg_packet) -> Result<Self, PacketInitError> {
		// println!("packet details: b_o_s={}, e_o_s={}", packet.b_o_s, packet.e_o_s);
        if let Err(usize_error) = usize::try_from(packet.bytes) {
			return Err(PacketInitError::Usize(usize_error))
		}

		// I don't quite understand this one. The documentation:
		// https://xiph.org/ogg/doc/libogg/ogg_packet.html
		// says that `1` indicates true for each of these values,
		// but the actual code disagrees and sets `b_o_s` to `256`
		// on true and `e_o_s` to `512` on true.
		if ![0, 256].contains(&packet.b_o_s) {
			return Err(PacketInitError::InvalidBeginningOfStream(packet.b_o_s))
		}
		if ![0, 512].contains(&packet.e_o_s) {
			return Err(PacketInitError::InvalidEndOfStream(packet.e_o_s))
		}

		Ok(Packet { packet, owned: None })
    }

	/// Return a reference to the data of this [Packet].
	pub fn data(&self) -> &[u8] {
		match &self.owned {
			None => unsafe { core::slice::from_raw_parts(self.packet.packet, self.packet.bytes as usize) },
			Some(owned_packet) => &owned_packet.data
		}
	}

	/// Return a reference to the data of this [Packet].
	pub fn data_mut(&mut self) -> &mut [u8] {
		match &mut self.owned {
			None => panic!("mutating a Packet which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned_packet) => &mut owned_packet.data
		}
	}

	/// Set the data of this `Packet`.
	pub fn set_data(&mut self, data: Vec<u8>) {
		match &mut self.owned {
			None => panic!("mutating a Packet which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned) => {
				if data.capacity() == 0 {
					owned.data = Vec::with_capacity(1)
				} else {
					owned.data = data;
				}
			}
		}
	}

	/// Check whether this packet begins a logical stream.
	pub fn begins_logical_stream(&self) -> bool {
		match &self.owned {
			None => self.packet.b_o_s != 0,
			Some(owned_packet) => owned_packet.beginning_of_stream
		}
	}

	/// Set whether this packet begins a logical stream.
	pub fn set_begins_local_stream(&mut self, begins_logical_stream: bool) {
		match self.owned.as_mut() {
			None => panic!("mutating a Packet which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned_packet) => owned_packet.beginning_of_stream = begins_logical_stream
		}
	}

	/// Check whether this packet ends a logical stream.
	pub fn ends_logical_stream(&self) -> bool {
		match &self.owned {
			None => self.packet.e_o_s != 0,
			Some(owned_packet) => owned_packet.end_of_stream
		}
	}

	/// Set whether this packet ends a logical stream.
	pub fn set_ends_local_stream(&mut self, ends_logical_stream: bool) {
		match self.owned.as_mut() {
			None => panic!("mutating a Packet which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned_packet) => owned_packet.end_of_stream = ends_logical_stream
		}
	}

	/// Return the absolute granule position of this packet.
	pub fn absgp(&self) -> u64 {
		match &self.owned {
			None => self.packet.granulepos as u64,
			Some(owned_packet) => owned_packet.absgp
		}
	}

	/// Set the absolute granule position of this packet.
	pub fn set_absgp(&mut self, absgp: u64) {
		match self.owned.as_mut() {
			None => panic!("mutating a Packet which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned_packet) => owned_packet.absgp = absgp
		}
	}

	/// Return the sequential number of this packet in the stream.
	pub fn index(&self) -> u32 {
		match &self.owned {
			None => self.packet.packetno as u32,
			Some(owned_packet) => owned_packet.index
		}
	}

	/// Set the sequential number of this packet in the stream.
	pub fn set_index(&mut self, index: u32) {
		match self.owned.as_mut() {
			None => panic!("mutating a Packet which is owned by ogg is illegal\nthis is a bug!"),
			Some(owned_packet) => owned_packet.index = index
		}
	}
}

impl Clone for Packet {
    fn clone(&self) -> Self {
		if let Some(owned) = &self.owned {
			let mut owned = owned.clone();
			Self { packet: owned.ogg_packet(), owned: Some(owned) }
		} else {
			let mut owned = PrivatePacket {
				data: self.data().to_vec(),
				beginning_of_stream: self.begins_logical_stream(),
				end_of_stream: self.ends_logical_stream(),
				absgp: self.absgp(),
				index: self.index()
			};
			let packet = owned.ogg_packet();
	        Self { packet, owned: Some(owned) }
		}
    }
}

/// Error while creating or verifying the [Packet] struct.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PacketInitError {
	/// Couldn't read a value as [usize].
	Usize (<usize as TryFrom<c_long>>::Error),
	/// Couldn't read a value as [u8].
	U8 (<u8 as TryFrom<c_long>>::Error),
	/// The beginning of stream flag is an invalid value.
	InvalidBeginningOfStream (c_long),
	/// The end of stream flag is an invalid value.
	InvalidEndOfStream (c_long)
}

impl std::fmt::Display for PacketInitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
			Self::Usize(usize_error) => write!(f, "couldn't read usize: {}", usize_error),
			Self::U8(u8_error) => write!(f, "couldn't read u8: {}", u8_error),
			Self::InvalidBeginningOfStream(i) => write!(f, "invalid beginning of stream flag: {} (should be 0 or 256)", i),
			Self::InvalidEndOfStream(i) => write!(f, "invalid end of stream flag: {} (should be 0 or 512)", i)
		}
    }
}
