# Investigar onde estao os dados do banco de dados
$locations = @(
    "$env:USERPROFILE\.cc-switch",
    "$env:APPDATA\cc-switch",
    "$env:APPDATA\com.cc-switch.desktop",
    "$env:APPDATA\com.ccswitch.desktop",
    "$env:LOCALAPPDATA\cc-switch",
    "$env:LOCALAPPDATA\com.cc-switch.desktop"
)

Write-Host "=== Procurando bancos de dados cc-switch ===" -ForegroundColor Yellow

foreach ($loc in $locations) {
    if (Test-Path $loc) {
        Write-Host "`n>>> Encontrado: $loc" -ForegroundColor Green
        Get-ChildItem -Path $loc -Recurse -Force -ErrorAction SilentlyContinue | 
            Select-Object FullName, Length, LastWriteTime |
            Format-Table -AutoSize
        
        # Procurar por arquivos .db
        $dbs = Get-ChildItem -Path $loc -Filter "*.db" -Recurse -ErrorAction SilentlyContinue
        foreach ($db in $dbs) {
            Write-Host "`n>>> Banco encontrado: $($db.FullName)" -ForegroundColor Cyan
            Write-Host "    Tamanho: $($db.Length) bytes"
            Write-Host "    Modificado: $($db.LastWriteTime)"
            
            # Tentar ler a versao do banco
            if (Get-Command sqlite3 -ErrorAction SilentlyContinue) {
                $ver = & sqlite3 $db.FullName "PRAGMA user_version;" 2>$null
                Write-Host "    Versao: $ver"
            }
        }
    }
}

# Procurar backups
Write-Host "`n=== Procurando backups ===" -ForegroundColor Yellow
Get-ChildItem -Path "$env:USERPROFILE\.cc-switch" -Filter "*.bak*" -Recurse -ErrorAction SilentlyContinue |
    Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize
Get-ChildItem -Path "$env:USERPROFILE\.cc-switch" -Filter "*.backup*" -Recurse -ErrorAction SilentlyContinue |
    Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize

# Procurar qualquer .db em toda a pasta
Write-Host "`n=== Todos os arquivos .db ===" -ForegroundColor Yellow
Get-ChildItem -Path "$env:USERPROFILE\.cc-switch" -Filter "*.db*" -Recurse -ErrorAction SilentlyContinue |
    Select-Object FullName, Length, LastWriteTime | Format-Table -AutoSize
