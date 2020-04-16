extern crate cc;
extern crate pkg_config;

#[cfg(feature = "bindgen")]
extern crate bindgen;

#[cfg(feature = "bindgen")]
#[path = "bindgen.rs"]
mod generate;

use std::env;
use std::path::PathBuf;

fn main() {
    #[cfg(feature = "bindgen")]
    generate::generate();

    let mut craft = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    craft.push("canonical-raft");
    craft.push("src");

    if !pkg_config::find_library("libraft").is_ok() {
        let mut builder = cc::Build::new();

        builder
            .files([
                craft.join("byte.c"),
                craft.join("client.c"),
                craft.join("configuration.c"),
                craft.join("convert.c"),
                craft.join("election.c"),
                craft.join("entry.c"),
                craft.join("err.c"),
                craft.join("fixture.c"),
                craft.join("heap.c"),
                craft.join("log.c"),
                craft.join("membership.c"),
                craft.join("progress.c"),
                craft.join("raft.c"),
                craft.join("recv.c"),
                craft.join("recv_append_entries.c"),
                craft.join("recv_append_entries_result.c"),
                craft.join("recv_install_snapshot.c"),
                craft.join("recv_request_vote.c"),
                craft.join("recv_request_vote_result.c"),
                craft.join("recv_timeout_now.c"),
                craft.join("replication.c"),
                craft.join("snapshot.c"),
                craft.join("start.c"),
                craft.join("state.c"),
                craft.join("syscall.c"),
                craft.join("tick.c"),
                craft.join("tracing.c"),
                craft.join("uv.c"),
                craft.join("uv_append.c"),
                craft.join("uv_encoding.c"),
                craft.join("uv_finalize.c"),
                craft.join("uv_fs.c"),
                craft.join("uv_ip.c"),
                craft.join("uv_list.c"),
                craft.join("uv_metadata.c"),
                craft.join("uv_os.c"),
                craft.join("uv_prepare.c"),
                craft.join("uv_recv.c"),
                craft.join("uv_segment.c"),
                craft.join("uv_send.c"),
                craft.join("uv_snapshot.c"),
                craft.join("uv_tcp.c"),
                craft.join("uv_tcp_connect.c"),
                craft.join("uv_tcp_listen.c"),
                craft.join("uv_truncate.c"),
                craft.join("uv_writer.c"),
            ].iter())
            .flag_if_supported("-Wno-unused-parameter")
            .flag_if_supported("-Wbad-function-cast")
            .flag_if_supported("-Wuninitialized");

        builder.compile("libraft.a")
    }
}
