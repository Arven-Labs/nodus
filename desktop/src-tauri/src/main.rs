// Não abre um console extra no Windows em release (o Windows ainda é Fase 2).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    nodus_desktop_lib::run()
}
