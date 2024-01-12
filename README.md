# ogg_xiph

This is an *incomplete* project which provides Rust bindings for libogg.

My goal was to create safe bindings to libogg to use in another project, but I
found a better way to do what I was doing and left this code. I struggled
to understand [the documentation](https://xiph.org/ogg/doc/libogg/index.html)
at times, and this is my first C FFI project, so it probably
*is not entirely safe.* It's also not very pretty. And there's probably a lot
of  pointless stuff, like the entire `PrivatePacket` struct. That being said,
I feel like letting it rot away in my list of unfinished projects would be
a shame, so it gets to rot on GitHub instead.

It is *not* well tested and you probably shouldn't use it.
See [ogg_pager](https://crates.io/crates/ogg_pager) instead if you want to read
and write Ogg files.

The following is from the (also incomplete)
[project documentation](https://strangejune.xyz/archive/ogg_xiph/ogg_xiph/index.html):

--------------------------------------------------------------------------------

This crate provides an interface to the [libogg library](https://xiph.org/ogg/).

This crate does *not* provide any function for decoding the data in an Ogg
stream. After you get packets, you'll need to use another library for decoding
them.

## Usage

The basic structure you need to know looks like this:

```text
            Ogg, the physical file or stream
                            V
Ogg pages, including metadata for assembling logical streams
                            V
         Ogg packets, the actual underlying data
```

You can start with a `SyncState`:

```rust
// The sync state may fail to initialize, so we should match that
let mut sync_state = match SyncState::new() {
	Err(()) => panic!("initializing sync state failed!"),
	Ok(sync_state) => sync_state
};
// And now we have a SyncState!
```

Then submit bytes to the `SyncState` to get `Pages` out of it:

```rust
// `&bytes` is a `&[u8]`.
let pages: Option<Vec<Page>> = match sync_state.submit_bytes(&bytes) {
	// Successfully gathered page(s)
	Ok(Some(pages)) => Some(pages),
	// Not enough data was available to build a page
	// This might also indicate an internal error
	Ok(None) => None,
	// An error occurred
	Err(page_write_error) => panic!("ogg returned an error: {}", page_write_error)
};
```

See the tests module for more examples.