use bindgen::callbacks::IntKind;
use bindgen::callbacks::ParseCallbacks;
use std::env;
use std::path::PathBuf;

#[derive(Debug)]
struct Callbacks;

impl ParseCallbacks for Callbacks {
    fn int_macro(&self, name: &str, _value: i64) -> Option<IntKind> {
        match name {
            // Does not work because inside of anonymous enum
            //  "RAFT_UNAVAILABLE"
            // | "RAFT_FOLLOWER"
            // | "RAFT_CANDIDATE"
            // | "RAFT_LEADER" => Some(IntKind::UShort),

              "RAFT_CANTBOOTSTRAP"
            | "RAFT_STANDBY"
            | "RAFT_VOTER"
            | "RAFT_SPARE" => Some(IntKind::Int),

              "RAFT_NOMEM"
            | "RAFT_MALFORMED" => Some(IntKind::Int),

            _ => Some(IntKind::UInt),
        }
    }
}

pub fn generate() {
    let mut craft = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    craft.push("canonical-raft");
    craft.push("include");

    let mut out_path = PathBuf::from(&env::var("CARGO_MANIFEST_DIR").unwrap());
    out_path.push("src");

    let bindings = bindgen::Builder::default()
        .header(craft.join("raft.h").to_string_lossy())
        .header(craft.join("raft").join("uv.h").to_string_lossy())
        .header(craft.join("raft").join("fixture.h").to_string_lossy())
        .whitelist_var("^(RAFT|raft)_.*")
        .whitelist_type("^(RAFT|raft)_.*")
        .whitelist_function("^(RAFT|raft)_.*")
        .size_t_is_usize(true)
        // .rustified_enum("RAFT_IO_.*")
        .ctypes_prefix("::libc")
        .blacklist_item("^uv_.*")
        // .blacklist_item("^__.*")
        .parse_callbacks(Box::new(Callbacks))
        .layout_tests(false)
        .prepend_enum_name(false)
        .rustfmt_bindings(true)
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
