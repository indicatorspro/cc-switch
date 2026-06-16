import pathlib

f = pathlib.Path(r"C:\GitHub\cc-switch\src-tauri\src\lib.rs")
content = f.read_text(encoding="utf-8")

old = """            commands::backends::list_backends,
            commands::backends::get_backend,
            commands::backends::create_backend,
            commands::backends::update_backend,
            commands::backends::delete_backend,
            commands::backends::start_backend,
            commands::backends::stop_backend,
            commands::backends::get_backend_status,"""

new = """            commands::backends::list_backends,
            commands::backends::get_backend,
            commands::backends::create_backend,
            commands::backends::update_backend,
            commands::backends::delete_backend,
            commands::backends::start_backend,
            commands::backends::stop_backend,
            commands::backends::restart_backend,
            commands::backends::get_backend_logs,
            commands::backends::send_backend_input,
            commands::backends::check_backend_health,
            commands::backends::list_backend_models,
            commands::backends::read_backend_env_file,
            commands::backends::write_backend_env_file,"""

count = content.count(old)
if count == 1:
    content = content.replace(old, new)
    f.write_text(content, encoding="utf-8")
    pathlib.Path(r"C:\GitHub\cc-switch\lib_result.txt").write_text("OK: replaced", encoding="utf-8")
else:
    # Try to find what's actually there
    idx = content.find("commands::backends::")
    if idx >= 0:
        ctx = content[idx:idx+500]
        pathlib.Path(r"C:\GitHub\cc-switch\lib_result.txt").write_text(f"Found {count} matches. Context:\n{ctx}", encoding="utf-8")
    else:
        pathlib.Path(r"C:\GitHub\cc-switch\lib_result.txt").write_text("NOT FOUND", encoding="utf-8")
