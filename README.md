# TaylorChat

Mensageiro **P2P offline** da suíte Taylor: mensagens diretas entre pares, **sem servidor, sem cadastro, sem número de telefone**. Identidade é uma chave; você pareia com alguém por QR/convite e conversa ponta a ponta. Feito em Tauri 2 + React + Rust, com IA local (llama.cpp) opcional pra redigir/resumir/traduzir.

> Nome fora do padrão `Local*` de propósito: um mensageiro **não** é "local" — o ponto dele é falar com outra máquina.

## Funcionalidades

- **Identidade** ed25519 gerada no 1º uso, guardada no cofre do SO (keyring) — sem cadastro.
- **Pareamento** por QR/convite (`taylorchat:<id>`), fora de banda.
- **Rede P2P** (iroh/QUIC): conexão direta autenticada, hole-punching, relays públicos de fallback (não veem conteúdo).
- **Cripto**: double ratchet (vodozemac/Olm) sobre o canal — forward secrecy; histórico SQLite **cifrado em repouso** (XChaCha20-Poly1305).
- **Arquivos**: anexar pelo 📎 ou **arrastar e soltar**; transferência **em streaming por chunks** (comprime/cifra 1 MiB por vez) → **arquivos grandes sem estourar a RAM, sem teto de tamanho**; **preview de imagem inline**.
- **Recibos**: ✓ enviado, ✓✓ entregue (ACK), ✓✓ lido (recibo de leitura); fila offline com **reenvio automático** quando o par volta.
- **UI**: prévia da última mensagem e não-lidos na lista, separadores de data, tema claro/escuro.
- **IA local** (llama.cpp, porta 8103): sugerir resposta, resumir conversa, melhorar/traduzir rascunho — a IA **só propõe**, nunca envia.

## Rodar

```bash
npm install
npm run tauri dev -- --features p2p   # com a rede (recomendado; é o build da release)
npm run tauri dev                     # build sem rede (UI/banco apenas)
```

IA local (opcional): baixe o runtime com `scripts/fetch-llama.ps1` (Win) / `scripts/fetch-llama.sh` (Linux) e aponte pra uma pasta de modelos `.gguf`. Modelos não inclusos.

## Roadmap

Plano completo e decisões: [`../projetos/plano.md`](../projetos/plano.md). Próximas fases: cripto forte (double ratchet), arquivos/mídia (zstd + iroh-blobs), IA, fila offline.

## Licença

MIT.
