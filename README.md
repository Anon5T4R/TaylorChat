# TaylorChat

Mensageiro **P2P offline** da suíte Taylor: mensagens diretas entre pares, **sem servidor, sem cadastro, sem número de telefone**. Identidade é uma chave; você pareia com alguém por QR/convite e conversa ponta a ponta. Feito em Tauri 2 + React + Rust, com IA local (llama.cpp) opcional pra redigir/resumir/traduzir.

> Nome fora do padrão `Local*` de propósito: um mensageiro **não** é "local" — o ponto dele é falar com outra máquina.

## Estado

`v0.1.0-dev` — scaffold com **Fases 1 e 2** prontas:

- **Identidade** ed25519 gerada no 1º uso, guardada no cofre do SO (keyring).
- **Banco** SQLite local (contatos, mensagens).
- **Pareamento** por convite/QR (gera o meu, adiciona por código/escaneio).
- **UI** de conversa (lista, bolhas, composer).

**Fase 3 (rede iroh)** está com o front pronto e o backend com `src/net.rs` atrás da feature `p2p`, a fechar num build real (ver `../projetos/plano.md`). Sem a feature, o app roda, pareia e guarda histórico; mensagens de saída ficam `queued`.

## Rodar

```bash
npm install
npm run tauri dev              # build padrão (sem rede ao vivo)
npm run tauri dev -- --features p2p   # com a rede iroh (Fase 3, a validar)
```

IA local (opcional): baixe o runtime com `scripts/fetch-llama.ps1` (Win) / `scripts/fetch-llama.sh` (Linux) e aponte pra uma pasta de modelos `.gguf`. Modelos não inclusos.

## Roadmap

Plano completo e decisões: [`../projetos/plano.md`](../projetos/plano.md). Próximas fases: cripto forte (double ratchet), arquivos/mídia (zstd + iroh-blobs), IA, fila offline.

## Licença

MIT.
