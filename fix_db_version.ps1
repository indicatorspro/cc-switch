# Script para redefinir a versão do banco de dados para v11
# Isso NÃO perde dados - apenas redefine o user_version

$dbPath = "$env:USERPROFILE\.cc-switch\cc-switch.db"

if (-not (Test-Path $dbPath)) {
    Write-Host "ERRO: Banco de dados nao encontrado em: $dbPath" -ForegroundColor Red
    exit 1
}

Write-Host "Banco encontrado: $dbPath" -ForegroundColor Green
Write-Host "Tamanho: $((Get-Item $dbPath).Length) bytes"

# Verificar se sqlite3 esta disponivel
$sqlite = Get-Command sqlite3 -ErrorAction SilentlyContinue
if ($sqlite) {
    Write-Host "Usando sqlite3 CLI..."
    & sqlite3 $dbPath "PRAGMA user_version;"
    & sqlite3 $dbPath "PRAGMA user_version = 11;"
    $ver = & sqlite3 $dbPath "PRAGMA user_version;"
    Write-Host "Versao definida para: $ver" -ForegroundColor Green
} else {
    Write-Host "sqlite3 nao encontrado. Usando Python..."
    python -c "
import sqlite3, sys
db = r'$($dbPath -replace "'","''")'
conn = sqlite3.connect(db)
cur = conn.cursor()
cur.execute('PRAGMA user_version')
old = cur.fetchone()[0]
print(f'Versao atual: {old}')
cur.execute('PRAGMA user_version = 11')
cur.execute('PRAGMA user_version')
new = cur.fetchone()[0]
print(f'Versao definida para: {new}')
conn.close()
print('Pronto! Banco de dados continuara funcionando com o binario v11.')
"
}
