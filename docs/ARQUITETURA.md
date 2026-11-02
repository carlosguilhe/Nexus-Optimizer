# Arquitetura

O Nexus Optimizer é um app **Tauri 2**: backend em **Rust** (acesso ao sistema) e interface em **React + TypeScript** (Vite) rodando no WebView2. A interface só chama comandos do backend por `invoke`; nada de sistema roda no front.

```
src/                      Interface (React + TS)
├─ App.tsx                Navegação, estado global, assistente e abertura
├─ tabs/                  Uma tela por aba (Início, Velocidade, Benchmark, Monitor, ...)
├─ components/            Componentes (lista de ajustes, perfis, rede, gráficos, splash, ...)
├─ theme.ts               10 temas = conjuntos de variáveis CSS
├─ i18n.tsx, locales/     Tradução (a chave é o texto em português)
└─ settings.tsx           Configurações sincronizadas com o backend

src-tauri/src/            Backend (Rust)
├─ tweaks.rs              Catálogo de ajustes, backup e reversão
├─ settings.rs            Configurações e perfis (settings.json)
├─ gamemode.rs, games.rs  Detecção de jogos, perfis por jogo, vigia em segundo plano
├─ bench.rs, history.rs   PresentMon, estatísticas, histórico, teste A/B, relatório HTML
├─ speedtest.rs           Teste de velocidade (eventos de progresso para o velocímetro)
├─ net.rs                 Servidores, ping, DNS, rota
├─ hw.rs                  GPU (nvidia-smi), discos, drivers, MSI, afinidade
├─ maint.rs, cleaner.rs   SFC/DISM, análise de disco, desinstalador, limpeza
├─ startup.rs, sys.rs     Inicialização, processos, uso de CPU/RAM
├─ shell.rs               Bandeja, atalhos globais, notificações, atualizações, agendador
└─ activity.rs            Histórico de atividades
```

## Princípios

1. **Backup antes de alterar.** `tweaks.rs` grava o valor original em `%APPDATA%\NexusOptimizer\backup.json` antes de qualquer escrita. Se a escrita falhar no meio, desfaz o que já tinha feito.
2. **Catálogo declarativo.** Cada ajuste declara `id`, categoria, risco, evidência honesta, entradas de registro (HKCU/HKLM) e se exige reiniciar. A interface é gerada a partir dele.
3. **Entradas validadas.** Comandos externos só recebem valores de listas fechadas (DNS, pacotes, alvos) ou validados (host, nome de processo). A interface nunca envia caminhos ou comandos.
4. **Processos protegidos.** O encerramento de processos recusa tudo dentro de `C:\Windows`, processos sem caminho legível e o próprio Nexus.
5. **Tudo bloqueante fora da interface.** Registro, PowerShell e rede rodam em `spawn_blocking`/async; progresso longo chega por eventos (`speedtest`, `maint`, `diskscan`, `trace`).

## Dados em disco

Em `%APPDATA%\NexusOptimizer\`: `backup.json` (valores originais), `settings.json`, `games.json` (catálogo), `bench_history.json`, `activity.jsonl`, `msi_backup.json`. Relatórios em `Documentos\Nexus Reports`, backups de configuração em `Documentos\Nexus Backups`.

## Testes

`cargo test --lib` cobre catálogo de ajustes (ids únicos, leitura de estado), ida e volta de escrita no registro, estatísticas de benchmark, veredito A/B, análise de saída do `ping`/`tracert`, pacotes DNS, varredura de arquivos, validação de entradas e mais. Testes que usam a internet ou alteram o sistema ficam em `#[ignore]` ou protegem o estado do usuário.
