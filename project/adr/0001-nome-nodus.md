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
- **Atualização de 2026-11-15 (ADR 0013):** o binário passou a se chamar `nodusctl` por causa da colisão de `PATH` com o crate `nodus`; o nome do projeto não mudou.
- **Atualização de 2026-11-21 (P-02, busca de marca, triagem por busca web, não parecer jurídico):** o USPTO tem dois registros `NODUS` vivos, da Nodus Technologies, Inc. (Anaheim, CA), nas classes 9 e 42, as mesmas do nodus: Reg. 2876650 (2004) e Reg. 3637558 (2009, renovado em 2019; segundo o Justia cobre software, manutenção de software e suporte técnico). O `NODUS CLOUD OS` (2021) foi abandonado após decisão inter partes. Fonte só agregadores (Justia, Markinton, TrademarkDir) em resumos de busca; as páginas não abriram (Justia 403, Markinton sem DNS), então status, lista exata de produtos e o que a empresa faz hoje **não estão confirmados**. EUIPO/TMview e INPI **não foram consultados** (buscadores interativos); no EUIPO só apareceu um `NODUS` lituano na classe 11, sem colisão. Web: nenhum produto `NODUS` de gestão de dispositivos ou infraestrutura; o mais próximo segue sendo o crate `nodus` (agentes de IA). Efeito: há risco de conflito de marca nos EUA, baixo enquanto a distribuição for só código-fonte (ADR 0010) e maior se divulgar ou publicar pacote. Decisão de manter o nome inalterada; reabrir se o TSDR confirmar os registros ativos e o dono quiser divulgar. no texto até a Fase 0 (P-03). **Resolvido em 2026-11-15:** o `SPEC.md` passou a dizer `nodus` (componentes `nodus Core`, `nodus Agent`, `nodus Desktop`, `nodus Web`), com uma nota no cabeçalho sobre a origem.
