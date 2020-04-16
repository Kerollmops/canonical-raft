#![allow(non_camel_case_types)]
#![allow(clippy::all)]
#![doc(html_root_url = "https://docs.rs/canonical-raft-sys/0.0.1")]

extern crate libc;
extern crate libuv_sys2;

use libuv_sys2::{uv_loop_s, uv_stream_s};

include!("bindings.rs");
