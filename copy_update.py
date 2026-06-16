import os
import shutil
import sys

src = r"C:\GitHub\cc-switch\cc-switch-UPDATE"
dst = r"C:\GitHub\cc-switch"

results = []

# Step 1: Copy UPDATE over main directory
copied = 0
for root, dirs, files in os.walk(src):
    dirs[:] = [d for d in dirs if d not in ('.git', 'target', 'node_modules', 'cc-switch-UPDATE')]
    
    rel = os.path.relpath(root, src)
    dst_dir = os.path.join(dst, rel)
    
    for f in files:
        src_file = os.path.join(root, f)
        dst_file = os.path.join(dst_dir, f)
        os.makedirs(dst_dir, exist_ok=True)
        shutil.copy2(src_file, dst_file)
        copied += 1

results.append(f"Step 1: Copied {copied} files from UPDATE")

# Step 2: Check if user's custom files survived
custom_files = [
    (r"C:\GitHub\cc-switch\src-tauri\src\database\backends.rs", "database/backends.rs"),
    (r"C:\GitHub\cc-switch\src-tauri\src\commands\backends.rs", "commands/backends.rs"),
]

for path, label in custom_files:
    if os.path.exists(path):
        size = os.path.getsize(path)
        results.append(f"  OK: {label} exists ({size} bytes)")
    else:
        results.append(f"  MISSING: {label} does NOT exist")

# Step 3: Check key files
key_files = [
    (r"C:\GitHub\cc-switch\src-tauri\src\database\schema.rs", "database/schema.rs"),
    (r"C:\GitHub\cc-switch\src-tauri\src\database\mod.rs", "database/mod.rs"),
    (r"C:\GitHub\cc-switch\src-tauri\src\commands\mod.rs", "commands/mod.rs"),
    (r"C:\GitHub\cc-switch\src-tauri\src\lib.rs", "lib.rs"),
]

for path, label in key_files:
    if os.path.exists(path):
        size = os.path.getsize(path)
        results.append(f"  OK: {label} exists ({size} bytes)")
    else:
        results.append(f"  MISSING: {label}")

with open(r"C:\GitHub\cc-switch\copy_result.txt", "w", encoding="utf-8") as f:
    f.write("\n".join(results))
