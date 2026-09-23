// The GTK UI implementation is split into ordered fragments under src/parts/ui.
// build.rs concatenates the fragments verbatim before Rust compiles this module.
include!(concat!(env!("OUT_DIR"), "/ui_impl.rs"));
