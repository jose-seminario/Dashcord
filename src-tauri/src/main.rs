// Consola de Windows habilitada en Release para inspección y diagnóstico en tiempo real:
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    println!("[BOOT] Iniciando proceso DashCord (Consola de diagnóstico activa)...");
    dashcord_lib::run();
}
