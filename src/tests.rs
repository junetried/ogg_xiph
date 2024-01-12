use crate::*;

/// Initialize the sync state.
#[test]
fn init_sync_state() {
    let sync_state = SyncState::new();

	if sync_state.is_err() {
		panic!("initializing sync state failed!")
	}
}

#[test]
fn sync_ogg_file() {
    let mut sync_state = match SyncState::new() {
		Err(()) => panic!("initializing sync state failed!"),
		Ok(sync_state) => sync_state
	};

	let mut found_first_page = false;
	let mut found_last_page = false;
	let mut first_stream: Option<i32> = None;
		
	let mut pages = match sync_state.submit_bytes(include_bytes!("../sine.ogg")) {
		Ok(Some(pages)) => {
			for page in &pages {
				if page.begins_logical_stream() {
					if found_first_page {
						panic!("found multiple first pages")
					} else {
						println!("found first page");
						found_first_page = true;
					}
				}
				if page.ends_logical_stream() {
					if found_last_page {
						panic!("found multiple last pages")
					} else {
						println!("found last page");
						found_last_page = true;
						println!("header: {:02X?}", page.header())
					}
				}
				if first_stream.is_none() {
					first_stream = Some(page.stream_serial())
				} else {
					if page.stream_serial() != first_stream.unwrap() {
						panic!("stream serial {} does not match the first stream serial {}", page.stream_serial(), first_stream.unwrap())
					}
				}
				//println!("page size: {} bytes, header size: {} bytes, stream: {}", page.data().len(), page.header().len(), page.stream_serial())
			}
			pages
		},
		Ok(None) => panic!("sync state returned no pages"),
		Err(page_error) => panic!("sync state returned an error: {}", page_error)
	};
	println!("found {} pages", pages.len());

	let mut stream = match Stream::new(first_stream.unwrap()) {
		Err(()) => panic!("initializing stream with serial {} returned error", first_stream.unwrap()),
		Ok(stream) => stream
	};

	println!("first stream serial: {}, as u32: {}", first_stream.unwrap(), first_stream.unwrap() as u32);

	let first_stream_serial_ogg = unsafe {
		ogg_next_sys::ogg_page_serialno(pages[0].ogg_page() as *const ogg_next_sys::ogg_page)
	};
	println!("first stream serial (ogg_page_serialno): {}", first_stream_serial_ogg);
	
	for (index, page) in pages.iter_mut().enumerate() {
		match stream.page_in(page) {
			Err(page_in_error) => panic!("stream returned an error: {} (added {} pages)", page_in_error, index),
			Ok(()) => {}
		}
	}

	let mut packets: Vec<Packet> = vec![];
	loop {
		match stream.packet_out() {
			Ok(packet) => packets.push(packet.clone()),
			Err(packet_out_error) => match packet_out_error  {
				PacketOutError::OutOfSync => panic!("stream.packet_out returned sync error"),
				PacketOutError::NoPages => unreachable!(),
				PacketOutError::InternalError => break
			}
		}
	}
	println!("found {} packets", packets.len())
}
