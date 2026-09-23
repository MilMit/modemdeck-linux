// The implementation is split into source-friendly fragments so GitHub and
// release tooling can handle the large module cleanly. build.rs concatenates
// the ordered fragments verbatim before Rust compiles this module.
include!(concat!(env!("OUT_DIR"), "/modem_manager_impl.rs"));
