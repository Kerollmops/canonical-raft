# canonical-raft

The, not released yet, simplest Rust library to replicate anything over the network.

## Current Limitations

Canonical-raft is based on [the raft library developed by canonical](https://github.com/canonical/raft) and the most important limitation [is that it requires Linux](https://github.com/canonical/raft/blob/master/README.md). This limitation is due to the fact that it uses the [AIO](http://man7.org/linux/man-pages/man2/io_submit.2.html) API for disk I/O, which is only available on Linux, [a pull request is opened to remove this limitation](https://github.com/canonical/raft/pull/119).

One other restraining element is that the currently provided `raft_io` runtime is based on `libuv` which is not the commonly used asynchronous runtime we use in the Rust community.

## Plans for canonical-raft

  1. [ ] [#1](https://github.com/Kerollmops/canonical-raft/issues/1) Create a safe abstraction on top of the bindings from canonical-raft-sys.
  2. [ ] [#2](https://github.com/Kerollmops/canonical-raft/issues/2) Create a `raft_io` interface that could run on top of any async runtime.
  3. [ ] [#3](https://github.com/Kerollmops/canonical-raft/issues/3) Create a canonical-raft-mdb akin to the [`MDBstore` of hashicorp](https://github.com/hashicorp/raft-mdb).

## Installation

This is a Rust library, yu must make sure to have [the toolchain installed](https://www.rust-lang.org/tools/install).

As it's explained above, canonical-raft depends on the raft library of canonical and is based on libuv.

```bash
apt install pkg-config libuv1-dev gcc make
cargo build
```

Note that it is highly recommended to run this library in release mode, for performances reasons.
