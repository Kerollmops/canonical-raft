# canonical-raft

The, not released yet, simplest Rust library to replicate anything over the network.

## Why This Library?

### The rust replication ecosystem is hard to use

There is many replication libraries in the Rust ecosystem but is there any one that is easy enough to use that anybody achieved to use it? And somebody that is not already the author of the replication library itself?

I don't think so!

Just look at the complexity of use of [the most popular Raft library developped by PingCAP](https://docs.rs/raft/0.5.0/raft/#creating-a-raft-node):
  1. You have to manage the Raft logic yourself, in your own loop.
  2. You have to deal with the Protocol Buffer messages.

### I am jealous of the Go replication ecosystem

Yes, I am talking about the Go programming language here, and more particularly about state replication over the network. The most popular library for replicating states is the HashiCorp one and why?

Look how easy it is:
  1. You just have to follow [the `FSM` interface](https://pkg.go.dev/github.com/hashicorp/raft?tab=doc#FSM).
  2. Your state will be replicated any time [you call `raft.Apply`](https://pkg.go.dev/github.com/hashicorp/raft?tab=doc#Raft.Apply).

That is just pure magic 🧙‍♀️💫

### About the wrapped canonical raft library

The canonical raft library was [mostly inspired by the HashiCorp one](https://github.com/canonical/raft#credits), and therefore inhertis its [simple interface](https://github.com/canonical/raft/blob/d9fd10016dc416f066bdead182c4e6973a4be047/example/server.c#L22-L90) where only three functions can be implemented.

This is why we must wrap it in the safest and easiest way as possible to make the Rust community proud of its replication ecosystem!

## Current Limitations

Canonical-raft is based on [the raft library developed by canonical](https://github.com/canonical/raft) and the most important limitation [is that it requires Linux](https://github.com/canonical/raft/blob/master/README.md). This limitation is due to the fact that it uses the [AIO](http://man7.org/linux/man-pages/man2/io_submit.2.html) API for disk I/O, which is only available on Linux, [a pull request is opened to remove this limitation](https://github.com/canonical/raft/pull/119).

One other restraining element is that the currently provided `raft_io` runtime is based on `libuv` which is not the commonly used asynchronous runtime we use in the Rust community.

## Plans for canonical-raft

  1. [ ] [#1](https://github.com/Kerollmops/canonical-raft/issues/1) Create a safe abstraction on top of the bindings from canonical-raft-sys.
  2. [ ] [#2](https://github.com/Kerollmops/canonical-raft/issues/2) Create a `raft_io` interface that could run on top of any async runtime.
  3. [ ] ~[#3](https://github.com/Kerollmops/canonical-raft/issues/3) Create a canonical-raft-mdb akin to the [`MDBstore` of HashiCorp](https://github.com/hashicorp/raft-mdb)~.

## Installation

This is a Rust library, you must make sure to have [the toolchain installed](https://www.rust-lang.org/tools/install).

As it's explained above, canonical-raft depends on [the raft library developed by canonical](https://github.com/canonical/raft) and it works with [libuv](libuv.org).

```bash
apt install pkg-config libuv1-dev gcc make
git submodule update --init --recursive
cargo build
```

Note that it is highly recommended to run this library in release mode, for performances reasons.

```bash
cargo build --release
```
