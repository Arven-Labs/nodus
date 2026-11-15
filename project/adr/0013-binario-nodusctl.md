# 0013: O binário se chama `nodusctl` (P-02)

**Data:** 2026-11-15

## Contexto
O ADR 0001 manteve o nome `nodus` ciente de homônimos. Medido em 2026-11-15 (P-02): o crate `nodus` 0.17.0 do crates.io (Apache-2.0, outro autor, CLI "local-first" para pacotes de agentes) declara `[[bin]] name = "nodus"` e se distribui por `cargo install`, script `curl | bash` e Homebrew. No Arch, `~/.cargo/bin` costuma vir antes de `/usr/bin` no `PATH`: quem instalar os dois ficaria com o dele, sem aviso, e o nosso comando perderia justamente para quem desenvolve.

## Decisão
1. O **binário** passa a se chamar `nodusctl` (`[[bin]]` em `crates/nodus-cli/Cargo.toml` e `name` do clap em `cli.rs`), pela convenção `*ctl` (`systemctl`, `kubectl`).
2. O projeto, o repositório, os crates (`nodus-core`, `nodus-cli`) e o pacote do AUR continuam `nodus`.
3. O diretório de estado `~/.local/state/nodus/` não muda (é do projeto, não do binário; evita mexer em dados reais).
4. ADRs e entradas antigas do `SESSIONS.md` não são reescritos: neles, "`nodus <comando>`" lê-se `nodusctl <comando>`.

## Alternativas
- **Manter `nodus` e só avisar no README:** descartada, a colisão é silenciosa para o usuário.
- **Renomear o projeto inteiro:** descartada por custo (repo, docs, CLI, ADRs); segue possível enquanto não houver tag nem pacote publicado (ADR 0001).
- **`ndus`:** descartada, parece erro de digitação e a disponibilidade não foi medida.

## Consequências
- Sem colisão de `PATH` com o crate homônimo; `nodusctl` está livre no crates.io e no AUR (medido em 2026-11-15).
- Quem já tinha um binário `nodus` compilado desta árvore fica com ele no `PATH` até remover (só o dono, pois não há binário distribuído).
- A confusão de busca e a marca continuam abertas na P-02.

## Não verificado
- `nodusctl` como **marca**; só medi crates.io, AUR e a máquina do dono.
- O binário real (build só no CI): o teste cobre o `name` do clap; o `[[bin]]` só se confere instalando.
