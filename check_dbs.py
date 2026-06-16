import sqlite3

def check_db(path, label):
    lines = []
    lines.append(f"=== {label} ===")
    lines.append(f"Path: {path}")
    try:
        conn = sqlite3.connect(path)
        cur = conn.cursor()
        cur.execute("PRAGMA user_version")
        ver = cur.fetchone()[0]
        lines.append(f"Versao: {ver}")
        cur.execute("SELECT name FROM sqlite_master WHERE type='table'")
        tables = [r[0] for r in cur.fetchall()]
        for t in tables:
            try:
                cur.execute(f'SELECT COUNT(*) FROM [{t}]')
                count = cur.fetchone()[0]
                lines.append(f"  {t}: {count} registros")
            except Exception as e:
                lines.append(f"  {t}: ERRO - {e}")
        conn.close()
    except Exception as e:
        lines.append(f"ERRO ao abrir: {e}")
    lines.append("")
    return "\n".join(lines)

db = r"C:\Users\Roberto Amaral\.cc-switch\cc-switch.db"
bak = r"C:\Users\Roberto Amaral\.cc-switch\backups\db_backup_20260615_222755.db"

result = check_db(db, "BANCO ATUAL")
result += check_db(bak, "BACKUP MAIS RECENTE")

with open(r"C:\GitHub\cc-switch\db_check_result.txt", "w", encoding="utf-8") as f:
    f.write(result)
