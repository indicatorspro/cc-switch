import sqlite3
import shutil
import os

db_path = r"C:\Users\Roberto Amaral\.cc-switch\cc-switch.db"
backup_path = r"C:\Users\Roberto Amaral\.cc-switch\backups\db_backup_20260615_222755.db"

# 1. Fazer backup do banco atual (safety)
shutil.copy2(db_path, db_path + ".before_restore")
print("Backup do banco atual salvo em: cc-switch.db.before_restore")

# 2. Restaurar o backup mais recente (v13, com mais dados)
shutil.copy2(backup_path, db_path)
print("Backup restaurado!")

# 3. Verificar
conn = sqlite3.connect(db_path)
cur = conn.cursor()
cur.execute("PRAGMA user_version")
ver = cur.fetchone()[0]
print(f"Versao do banco restaurado: {ver}")

cur.execute("SELECT name FROM sqlite_master WHERE type='table'")
tables = [r[0] for r in cur.fetchall()]
for t in tables:
    try:
        cur.execute(f"SELECT COUNT(*) FROM [{t}]")
        count = cur.fetchone()[0]
        print(f"  {t}: {count} registros")
    except:
        pass

# 4. Verificar managed_backends especificamente
print("\n=== managed_backends (gestor de proxy) ===")
try:
    cur.execute("SELECT * FROM managed_backends")
    rows = cur.fetchall()
    cur.execute("PRAGMA table_info(managed_backends)")
    cols = [r[1] for r in cur.fetchall()]
    print(f"Colunas: {cols}")
    print(f"Registros: {len(rows)}")
    for row in rows:
        print(f"  {row}")
except Exception as e:
    print(f"ERRO: {e}")

conn.close()
print("\nPronto! Reinicie o app.")
