# Script para investigar e restaurar backups do cc-switch
$ccDir = "$env:USERPROFILE\.cc-switch"

Write-Host "=== Diretorio cc-switch ===" -ForegroundColor Yellow
if (Test-Path $ccDir) {
    Write-Host "Encontrado: $ccDir"
    Get-ChildItem $ccDir -Force | ForEach-Object {
        Write-Host "  $($_.Name)  ($($_.Length) bytes, $($_.LastWriteTime))"
    }
} else {
    Write-Host "NAO ENCONTRADO!" -ForegroundColor Red
}

Write-Host "`n=== Backups do banco de dados ===" -ForegroundColor Yellow
$backupDir = "$ccDir\backups"
if (Test-Path $backupDir) {
    $backups = Get-ChildItem $backupDir -Filter "*.db" -Force | Sort-Object LastWriteTime -Descending
    if ($backups.Count -gt 0) {
        Write-Host "Encontrados $($backups.Count) backups:" -ForegroundColor Green
        foreach ($b in $backups) {
            Write-Host "  $($b.Name)  ($($b.Length) bytes, $($b.LastWriteTime))"
        }
        
        # Mostrar o mais recente
        $latest = $backups[0]
        Write-Host "`n>>> Backup mais recente: $($latest.FullName)" -ForegroundColor Cyan
        Write-Host "    Tamanho: $($latest.Length) bytes"
        Write-Host "    Data: $($latest.LastWriteTime)"
        
        # Ler tabelas do backup
        Write-Host "`n>>> Conteudo do backup:" -ForegroundColor Cyan
        python -c "
import sqlite3
conn = sqlite3.connect(r'$($latest.FullName)')
cur = conn.cursor()

# Listar tabelas
cur.execute(""SELECT name FROM sqlite_master WHERE type='table'"")
tables = [r[0] for r in cur.fetchall()]
print(f'Tabelas: {tables}')

for t in tables:
    try:
        cur.execute(f'SELECT COUNT(*) FROM [{t}]')
        count = cur.fetchone()[0]
        print(f'  {t}: {count} registros')
    except:
        print(f'  {t}: (erro ao contar)')
conn.close()
"
    } else {
        Write-Host "Nenhum backup encontrado!" -ForegroundColor Red
    }
} else {
    Write-Host "Diretorio de backups nao existe: $backupDir" -ForegroundColor Red
}

# Verificar banco atual
Write-Host "`n=== Banco de dados atual ===" -ForegroundColor Yellow
$dbPath = "$ccDir\cc-switch.db"
if (Test-Path $dbPath) {
    Write-Host "Encontrado: $dbPath  ($((Get-Item $dbPath).Length) bytes)"
    python -c "
import sqlite3
conn = sqlite3.connect(r'$dbPath')
cur = conn.cursor()
cur.execute('PRAGMA user_version')
print(f'Versao: {cur.fetchone()[0]}')
cur.execute(""SELECT name FROM sqlite_master WHERE type='table'"")
tables = [r[0] for r in cur.fetchall()]
for t in tables:
    try:
        cur.execute(f'SELECT COUNT(*) FROM [{t}]')
        count = cur.fetchone()[0]
        print(f'  {t}: {count} registros')
    except:
        print(f'  {t}: (erro)')
conn.close()
"
} else {
    Write-Host "Banco de dados NAO ENCONTRADO!" -ForegroundColor Red
}
