// Bez okna konsoli w wersji Windows
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    zarzadca_lib::run()
}
