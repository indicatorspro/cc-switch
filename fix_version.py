import sqlite3

db_path = r"C:\Users\Roberto Amaral\.cc-switch\cc-switch.db"

conn = sqlite3.connect(db_path)
cur = conn.cursor()

# Verificar versao atual
cur.execute("PRAGMA user_version")
old = cur.fetchone()[0]
print(f"Versao ANTES: {old}")

# Redefinir para 11 (binario atual suporta apenas v11)
cur.execute("PRAGMA user_version = 11")

# Verificar
cur.execute("PRAGMA user_version")
new = cur.fetchone()[0]
print(f"Versao DEPOIS: {new}")

# Confirmar dados intactos
cur.execute("SELECT COUNT(*) FROM managed_backends")
print(f"managed_backends: {cur.fetchone()[0]} registros")
cur.execute("SELECT COUNT(*) FROM proxy_config")
print(f"proxy_config: {cur.fetchone()[0]} registros")
cur.execute("SELECT COUNT(*) FROM providers")
print(f"providers: {cur.fetchone()[0]} registros")

conn.close()
print("\nPronto! O app deve abrir agora.")
