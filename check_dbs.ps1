# Verificar conteudo do backup e do banco atual
$backupPath = "$env:USERPROFILE\.cc-switch\backups\db_backup_20260615_222755.db"
$dbPath = "$env:USERPROFILE\.cc-switch\cc-switch.db"

python3 -c "
import sqlite3, sys

def check_db(path, label):
    print(f'=== {label} ===')
    print(f'Path: {path}')
    try:
        conn = sqlite3.connect(path)
        cur = conn.cursor()
        
        cur.execute('PRAGMA user_version')
        ver = cur.fetchone()[0]
        print(f'Versao: {ver}')
        
        cur.execute(\"SELECT name FROM sqlite_master WHERE type='table'\")
        tables = [r[0] for r in cur.fetchall()]
        
        for t in tables:
            try:
                cur.execute(f'SELECT COUNT(*) FROM [{t}]')
                count = cur.fetchone()[0]
                print(f'  {t}: {count} registros')
            except Exception as e:
                print(f'  {t}: ERRO - {e}')
        conn.close()
    except Exception as e:
        print(f'ERRO ao abrir: {e}')
    print()

check_db(r'$dbPath', 'BANCO ATUAL')
check_db(r'$backupPath', 'BACKUP MAIS RECENTE')
" 2>&1
