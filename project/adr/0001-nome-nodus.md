# 0001: Nome do projeto: nodus

**Data:** 2026-11-12

## Contexto
A spec chamava o projeto de "Universal", declaradamente provisório e genérico demais para repositório, pacote e CLI.

## Decisão
O projeto se chama **nodus** (`Arven-Labs/nodus`).

## Alternativas
- `universal`: nome provisório da spec; genérico, sem identidade e impossível de buscar.

## Consequências
- Nome curto, que remete a nó (dispositivo, servidor, ponto de uma frota).
- Risco aceito: "nodus" é uma palavra latina comum e há projetos homônimos. Marca e domínio em aberto (`PENDING.md`, P-02).
- **Atualização de 2026-11-15 (P-02):** o nome foi mantido (opção A). Medido: o nome `nodus` está **ocupado no crates.io** (CLI "local-first" para pacotes de agentes, de outro autor), no npm e no PyPI; livre no AUR e nos repositórios do Arch; `nodus-core` e `nodus-cli` livres no crates.io. A distribuição é só código-fonte (ADR 0010), então o conflito do crates.io pesa pouco; busca de marca, conflito do binário e domínio seguem abertos na P-02. Se a marca ou o binário se mostrarem um problema, reabrir este ADR; hoje, sem tag nem pacote publicado, é o momento mais barato de renomear.
- A spec mantém "Universal" no texto até a Fase 0 (P-03).
