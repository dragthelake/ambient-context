// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // The check happens before anything Tauri touches. `mcp` must run in a
    // bare process with no window server, no event loop and no config
    // directory, because an MCP client launches it as a subprocess on a
    // machine where the app may never have been opened.
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() == Some("mcp") {
        ambient_context_lib::mcp::run_stdio(
            ambient_context_lib::mcp::config_dir(),
            ambient_context_lib::mcp::app_data_dir(),
        );
    }
    ambient_context_lib::run()
}
