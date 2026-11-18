#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(unix)]
    if starpersona_lib::native_host() {
        return;
    }
    starpersona_lib::run()
}
