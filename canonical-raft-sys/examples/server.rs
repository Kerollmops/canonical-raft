use std::convert::TryInto;
use std::ffi::{CStr, CString};
use std::{mem, ptr, str};

use canonical_raft_sys::*;
use libc::{c_void, c_char, c_int, c_uint};
use libuv_sys2::{UV_VERSION_MAJOR, UV_VERSION_MINOR, UV_VERSION_PATCH};
use libuv_sys2::{uv_handle_t, uv_timer_t};
use libuv_sys2::{uv_default_loop, uv_run_mode_UV_RUN_DEFAULT};
use libuv_sys2::{uv_run, uv_close, uv_loop_init, uv_loop_close, uv_timer_init, uv_timer_start};
use libuv_sys2::{uv_signal_init, uv_signal_start, uv_signal_stop};
use libuv_sys2::{uv_loop_s, uv_signal_s, uv_timer_s};

// Number of servers in the example cluster
const N_SERVERS: usize = 3;

// Apply a new entry every 125 milliseconds
const APPLY_RATE: u64 = 125;

/********************************************************************
 *
 * Sample application FSM that just increases a counter.
 *
 ********************************************************************/

unsafe fn slice_from_buf<'a>(buf: raft_buffer) -> &'a [u8] {
    std::slice::from_raw_parts(buf.base as *const u8, buf.len)
}

#[repr(C)]
struct Fsm {
    count: u64,
}

unsafe extern "C" fn fsm_apply(
    fsm: *mut raft_fsm,
    buf: *const raft_buffer,
    result: *mut *mut c_void,
) -> c_int
{
    let f: &mut Fsm = mem::transmute((*fsm).data);

    println!("Hello fsm_apply");

    let buf = slice_from_buf(*buf);
    if buf.len() != mem::size_of::<u64>() {
        return RAFT_MALFORMED;
    }

    f.count += buf.try_into().map(u64::from_ne_bytes).unwrap();
    (*result) = &mut f.count as *mut _ as *mut c_void;

    println!("count after apply: {}", f.count);

    return 0;
}

unsafe extern "C" fn fsm_snapshot(
    fsm: *mut raft_fsm,
    bufs: *mut *mut raft_buffer,
    n_bufs: *mut c_uint,
) -> c_int
{
    let f: &mut Fsm = mem::transmute((*fsm).data);

    println!("Hello fsm_snapshot (count {})", f.count);

    *n_bufs = 1;
    *bufs = raft_malloc(mem::size_of::<raft_buffer>()) as *mut raft_buffer;
    if (*bufs).is_null() {
        return RAFT_NOMEM;
    }

    // (*bufs)[0]
    (**bufs).len = mem::size_of::<u64>();
    (**bufs).base = raft_malloc((**bufs).len);

    if (**bufs).base.is_null() {
        raft_free((*bufs) as *mut _); // avoid leaking!
        return RAFT_NOMEM;
    }
    ptr::copy_nonoverlapping(&f.count, (**bufs).base as *mut u64, 1);

    return 0;
}

unsafe extern "C" fn fsm_restore(fsm: *mut raft_fsm, buf: *mut raft_buffer) -> c_int {
    let f: &mut Fsm = mem::transmute((*fsm).data);

    println!("Hello fsm_restore");

    let slice = slice_from_buf(*buf);
    if slice.len() != mem::size_of::<u64>() {
        return RAFT_MALFORMED;
    }

    f.count = slice.try_into().map(u64::from_ne_bytes).unwrap();

    raft_free((*buf).base);

    println!("bye! fsm_restore (count {})", f.count);

    return 0;
}

unsafe extern "C" fn fsm_init(fsm: *mut raft_fsm) -> c_int {
    let f: *mut Fsm = raft_malloc(mem::size_of::<Fsm>()) as *mut _;
    if f.is_null() {
        return RAFT_NOMEM;
    }

    (*f).count = 0;
    (*fsm).version = 1;
    (*fsm).data = f as *mut c_void;
    (*fsm).apply = Some(fsm_apply);
    (*fsm).snapshot = Some(fsm_snapshot);
    (*fsm).restore = Some(fsm_restore);

    return 0;
}

unsafe extern "C" fn fsm_close(fsm: *mut raft_fsm) {
    println!("Hello fsm_close");

    if !(*fsm).data.is_null() {
        raft_free((*fsm).data);
    }
}

/********************************************************************
 *
 * Example struct holding a single raft server instance and all its
 * dependencies.
 *
 ********************************************************************/

unsafe extern "C" fn server_raft_close_cb(raft: *mut raft) {
    let s: &mut Server = mem::transmute((*raft).data);

    raft_uv_close(&mut s.io);
    raft_uv_tcp_close(&mut s.transport);
    fsm_close(&mut s.fsm);

    if let Some(close_cb) = s.close_cb {
        close_cb(s);
    }
}

unsafe extern "C" fn server_transfer_cb(req: *mut raft_transfer) {
    let s: &mut Server = mem::transmute((*req).data);

    let mut id: raft_id = 0;
    let mut address: *const c_char = ptr::null();
    raft_leader(&mut s.raft, &mut id, &mut address);
    raft_close(&mut s.raft, Some(server_raft_close_cb));
}

/// Final callback in the shutdown sequence, invoked after the timer handle has been closed.
unsafe extern "C" fn server_timer_close_cb(handle: *mut uv_handle_t) {
    let s: &mut Server = mem::transmute((*handle).data);

    if !s.raft.data.is_null() {
        if s.raft.state == RAFT_LEADER as u16 {
            let rv = raft_transfer(&mut s.raft, &mut s.transfer, 0, Some(server_transfer_cb));
            if rv == 0 { return }
        }
        raft_close(&mut s.raft, Some(server_raft_close_cb));
    }
}

#[repr(C)]
struct Server {
    data: *mut c_void,               // User data context.
    loop_: *mut uv_loop_s,           // UV loop.
    timer: uv_timer_s,               // To periodically apply a new entry.
    dir: *const c_char,              // Data dir of UV I/O backend.
    transport: raft_uv_transport,    // UV I/O backend transport.
    io: raft_io,                     // UV I/O backend.
    fsm: raft_fsm,                   // Sample application FSM.
    id: u32,                         // Raft instance ID.
    address: [u8; 64],               // Raft instance address.
    raft: raft,                      // Raft instance.
    transfer: raft_transfer,         // Transfer leadership request.
    close_cb: Option<ServerCloseCb>, // Optional close callback.
}

// typedef void (*ServerCloseCb)(struct Server *server);
type ServerCloseCb = unsafe extern "C" fn(*mut Server);

/// Initialize the example server struct, without starting it yet.
unsafe fn server_init(
    s: *mut Server,
    loop_: *mut uv_loop_s,
    dir: *const c_char,
    id: u32,
) {
    // // Seed the random generator
    // timespec_get(&now, TIME_UTC);
    // srandom((unsigned)(now.tv_nsec ^ now.tv_sec));
    // unimplemented!();

    ptr::write_bytes(s, 0, 1);

    (*s).loop_ = loop_;

    // Add a timer to periodically try to propose a new entry.
    let rv = uv_timer_init((*s).loop_, &mut (*s).timer);
    if rv != 0 {
        // Logf(s->id, "uv_timer_init(): %s", uv_strerror(rv));
        // goto err;
        panic!("Whoops...");
    }
    (*s).timer.data = s as *mut _;

    // Initialize the TCP-based RPC transport.
    let rv = raft_uv_tcp_init(&mut (*s).transport, (*s).loop_);
    if rv != 0 {
        // goto err;
        panic!("Ouïe!!!");
    }

    // Initialize the libuv-based I/O backend.
    let rv = raft_uv_init(&mut (*s).io, (*s).loop_, dir, &mut (*s).transport);
    if rv != 0 {
        // Logf(s->id, "raft_uv_init(): %s", s->io.errmsg);
        // goto err_after_uv_tcp_init;
        panic!("Aïeeeuux!!!");
    }

    // Initialize the finite state machine.
    let rv = fsm_init(&mut (*s).fsm);
    if rv != 0 {
        // Logf(s->id, "FsmInit(): %s", raft_strerror(rv));
        // goto err_after_uv_init;
        panic!("Aïe");
    }

    // Save the server ID.
    (*s).id = id;

    // Render the address.
    let address = CString::new(format!("127.0.0.1:900{}", id)).unwrap();

    let bytes = address.as_bytes_with_nul();
    (*s).address[..bytes.len()].copy_from_slice(bytes);

    let rv = raft_init(&mut (*s).raft, &mut (*s).io, &mut (*s).fsm, id.into(), address.as_ptr());
    if rv != 0 {
        let errmsg = unsafe {
            // This is safe since the error messages returned from mdb_strerror are static.
            let err: *const c_char = raft_errmsg(&mut (*s).raft);
            str::from_utf8_unchecked(CStr::from_ptr(err).to_bytes())
        };
        eprintln!("{}: raft_init(): {}", id, errmsg);
        panic!("AAA");
    }
    (*s).raft.data = s as *mut _;

    // Bootstrap the initial configuration if needed.
    let mut configuration = mem::zeroed();
    raft_configuration_init(&mut configuration);

    for i in 0..N_SERVERS {
        let server_id = (i + 1) as u64;
        let address = CString::new(format!("127.0.0.1:900{}", server_id)).unwrap();
        let rv = raft_configuration_add(&mut configuration, server_id, address.as_ptr(), RAFT_VOTER);
        if rv != 0 {
            // Logf(s->id, "raft_configuration_add(): %s", raft_strerror(rv));
            // goto err_after_configuration_init;
            panic!("Ouuups...");
        }
    }

    let rv = raft_bootstrap(&mut (*s).raft, &configuration);
    if rv != 0 && rv != RAFT_CANTBOOTSTRAP {
        // goto err_after_configuration_init;
        panic!("Bubuut?! Cant bootstrap raft (address already in use???)");
    }

    raft_configuration_close(&mut configuration);
    drop(configuration);

    raft_set_snapshot_threshold(&mut (*s).raft, 64);
    raft_set_snapshot_trailing(&mut (*s).raft, 16);

    (*s).transfer.data = s as *mut _;

    // err_after_configuration_init:
    //     raft_configuration_close(&configuration);
    // err_after_fsm_init:
    //     FsmClose(&s->fsm);
    // err_after_uv_init:
    //     raft_uv_close(&s->io);
    // err_after_uv_tcp_init:
    //     raft_uv_tcp_close(&s->transport);
    // err:
    //     return rv;
}

/// Called after a request to apply a new command to the FSM has been completed.
unsafe extern "C" fn server_apply_cb(req: *mut raft_apply, status: i32, result: *mut c_void) {
    let s: &mut Server = mem::transmute((*req).data);

    raft_free(req as *mut _);

    println!("{}: Hello server_apply_cb (status: {})", s.id, status);

    if status != 0 {
        if status != RAFT_LEADERSHIPLOST as i32 {
            let errmsg = unsafe {
                // This is safe since the error messages returned from mdb_strerror are static.
                let err: *const c_char = raft_errmsg(&mut (*s).raft);
                str::from_utf8_unchecked(CStr::from_ptr(err).to_bytes())
            };
            println!("{}: raft_apply() callback: {} ({})", s.id, errmsg, status);
        }
        return;
    }

    let count = *(result as *const i32);

    // if count % 100 == 0 {
        println!("{}: count {}", s.id, count);
    // }
}

/// Called periodically every APPLY_RATE milliseconds.
unsafe extern "C" fn server_timer_cb(timer: *mut uv_timer_t) {
    let s: &mut Server = mem::transmute((*timer).data);

    if s.raft.state != RAFT_LEADER as u16 {
        // println!("{}: not leader, skipping", s.id);
        return;
    }

    println!("{}: I am the leader", s.id);

    let buf = raft_buffer {
        len: mem::size_of::<u64>(),
        base: raft_malloc(mem::size_of::<u64>()),
    };

    if buf.base.is_null() {
        // Log(s->id, "serverTimerCb(): out of memory");
        return;
    }

    // write 1u64 into base
    ptr::copy_nonoverlapping(&1u64, buf.base as *mut u64, 1);

    let req = raft_malloc(mem::size_of::<raft_apply>()) as *mut raft_apply;
    if req.is_null() {
        // Log(s->id, "serverTimerCb(): out of memory");
        return;
    }
    (*req).data = s as *mut Server as *mut c_void;

    eprintln!("before raft_apply");

    let rv = raft_apply(&mut (*s).raft, req, &buf, 1, Some(server_apply_cb));
    if rv != 0 {
        // Logf(s->id, "raft_apply(): %s", raft_errmsg(&s->raft));
        return;
    }

    eprintln!("bye server_timer_cb");
}

/// Start the example server.
unsafe fn server_start(s: *mut Server) -> i32 {
    println!("{}: starting", (*s).id);

    let rv = raft_start(&mut (*s).raft);
    if rv != 0 {
        // eprintln!("{}: raft_start(): {}", s->id, raft_errmsg(&s->raft));
        return rv;
    }

    let rv = uv_timer_start(&mut (*s).timer, Some(server_timer_cb), 0, APPLY_RATE);
    if rv != 0 {
        // Logf(s->id, "uv_timer_start(): %s", uv_strerror(rv));
        return rv;
    }

    return 0;
}

/// Release all resources used by the example server.
unsafe fn server_close(s: &mut Server, cb: ServerCloseCb) {
    s.close_cb = Some(cb);

    println!("{}: stopping", s.id);

    // Close the timer asynchronously if it was successfully
    // initialized. Otherwise invoke the callback immediately.
    if !s.timer.data.is_null() {
        uv_close(&mut s.timer as *mut _ as *mut uv_handle_t, Some(server_timer_close_cb));
    } else {
        if let Some(close_cb) = s.close_cb {
            close_cb(s);
        }
    }
}

/********************************************************************
 *
 * Top-level main loop.
 *
 ********************************************************************/

unsafe extern "C" fn main_server_close_cb(server: *mut Server) {
    let sigint = (*server).data as *mut uv_handle_t;
    uv_close(sigint, None);
}

/// Handler triggered by SIGINT. It will initiate the shutdown sequence.
unsafe extern "C" fn main_sigint_cb(handle: *mut uv_signal_s, signum: i32) {
    let server: &mut Server = mem::transmute((*handle).data);
    assert_eq!(signum, libc::SIGINT);
    uv_signal_stop(handle);
    server.data = handle as *mut _;
    server_close(server, main_server_close_cb);
}

fn main() {
    let mut args = std::env::args();
    let dir = CString::new(args.nth(1).unwrap()).unwrap();
    let id: u32 = args.next().unwrap().parse().unwrap();

    // Ignore SIGPIPE, see https://github.com/joyent/libuv/issues/1254
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_IGN) };

    // println!("UV_VERSION_MAJOR {}.{}.{}", UV_VERSION_MAJOR, UV_VERSION_MINOR, UV_VERSION_PATCH);

    // Initialize the libuv loop.
    let loop_ = unsafe { uv_default_loop() };
    // let mut loop_ = mem::MaybeUninit::zeroed();
    // let rv = unsafe { uv_loop_init(loop_.as_mut_ptr()) };
    // if rv != 0 {
    //     // Logf(id, "uv_loop_init(): %s", uv_strerror(rv));
    //     // goto err;
    //     panic!("Whoops for uv_loop_init");
    // }
    // let mut loop_ = unsafe { loop_.assume_init() };

    // Initialize the example server.
    let mut server = unsafe { mem::zeroed() };
    unsafe { server_init(&mut server, loop_, dir.as_ptr(), id) };
    // let mut server = Server::new(loop_, dir.as_ptr(), id);

    // // Add a signal handler to stop the example server upon SIGINT.
    // let mut sigint = mem::MaybeUninit::zeroed();
    // let rv = unsafe { uv_signal_init(loop_, sigint.as_mut_ptr()) };
    // if rv != 0 {
    //     // Logf(id, "uv_signal_init(): %s", uv_strerror(rv));
    //     // goto err_after_server_init;
    //     panic!("Cannot uv_signal_init");
    // }
    // let mut sigint = unsafe { sigint.assume_init() };
    // sigint.data = &mut server as *mut _ as *mut c_void;

    // let rv = unsafe { uv_signal_start(&mut sigint, Some(main_sigint_cb), libc::SIGINT) };
    // if rv != 0 {
    //     // Logf(id, "uv_signal_start(): %s", uv_strerror(rv));
    //     // goto err_after_signal_init;
    //     panic!("Cannot uv_signal_start");
    // }

    // Start the server.
    let rv = unsafe { server_start(&mut server) };
    if rv != 0 {
        panic!("server start error");
        // goto err_after_signal_init;
    }

    // Run the event loop until we receive SIGINT.
    let rv = unsafe { uv_run(loop_, uv_run_mode_UV_RUN_DEFAULT) };
    if rv != 0 {
        // Logf(id, "uv_run_start(): %s", uv_strerror(rv));
        panic!("Whoops uv_loop_start");
    }

    unsafe { uv_loop_close(loop_) };

// err_after_signal_init:
//     uv_close((struct uv_handle_s *)&sigint, NULL);
// err_after_server_init:
//     ServerClose(&server, NULL);
//     uv_run(&loop, UV_RUN_DEFAULT);
//     uv_loop_close(&loop);
// err:
//     return rv;
}
