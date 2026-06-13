# Workflow de Atualização - CC Switch Custom

## Estrutura de Branches

| Branch | Função |
|---|---|
| `main` | Branch de trabalho. Recebe atualizações do upstream + suas customizações |
| `custom` | Backup/snapshot do estado atual. Atualizado manualmente após cada merge |
| `upstream/main` | Remote tracking do repositório oficial (`farion1231/cc-switch`) |

## Suas Alterações Customizadas

| Arquivo | O que foi alterado |
|---|---|
| `src-tauri/src/database/mod.rs` | `SCHEMA_VERSION` definido como 13 (upstream está em 11) |
| `src-tauri/src/database/schema.rs` | Migrações v11→v12 (api_key em managed_backends) e v12→v13 (colunas pricing_model/request_model) |
| `src-tauri/src/database/backends.rs` | Funcionalidade de backends gerenciados (tabela managed_backends) |
| `src-tauri/src/commands/backends.rs` | Comandos Tauri para gerenciar backends via UI |
| `src-tauri/src/services/backend_runtime.rs` | Runtime para iniciar/parar proxies gerenciados |

## Como Atualizar com o Upstream

### 1. Verificar se há atualizações
```powershell
cd C:\GitHub\cc-switch
git fetch upstream
git log main..upstream/main --oneline
```

### 2. Fazer o merge
```powershell
git merge upstream/main
```

### 3. Resolver conflitos (se houver)
- O Git pausa automaticamente se houver conflitos
- **NUNCA aceite as mudanças do upstream sem revisar** — ele pode reverter suas customizações
- Arquivos que normalmente dão conflito: `mod.rs`, `schema.rs`
- Mantenha sua versão quando o upstream não inclui suas funcionalidades personalizadas

### 4. Atualizar a branch de backup
```powershell
git checkout custom
git merge main
git checkout main
```

### 5. Testar
```powershell
pnpm run dev
```

### 6. Compilar e instalar
```powershell
pnpm tauri build
```

## ⚠️ Regras Importantes

1. **Sempre faça backup antes de merge**: `git checkout custom && git merge main`
2. **Nunca force push**: `git push` sem `--force`
3. **Revise cada conflito manualmente**: Não aceite cegamente as mudanças do upstream
4. **Teste sempre após merge**: rode `pnpm run dev` antes de compilar
5. **Documente novas customizações**: adicione à tabela acima quando modificar arquivos
