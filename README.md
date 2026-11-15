# nodus

Plataforma open source e extensível para conhecer, configurar, padronizar, monitorar e administrar computadores, servidores, VPS, ambientes de desenvolvimento e aplicações em um único lugar. Local-first, self-hostable e independente de fabricante.

**Status:** em desenvolvimento, antes do primeiro release (nenhuma tag ainda). Há uma CLI em Rust (`nodus`) para Arch Linux, sem GUI, com estes comandos:

- `nodusctl inventory`: sistema, hardware e pacotes (pacman e Flatpak), com `--json`.
- `nodusctl updates`: atualizações pendentes (pacman e Flatpak).
- `nodusctl repos <pasta>`: estado dos repositórios Git da pasta, lendo só refs.
- `nodusctl preset init | plan | apply`: descrever os pacotes desejados em TOML, ver o que falta e instalar o que falta do pacman, com confirmação. Apps Flatpak entram no `plan`, mas o `apply` ainda não os instala.
- `nodusctl history` e `nodusctl history undo <n>`: listar as execuções do `apply` (o que foi pedido, não as dependências que vieram junto) e desfazer uma instalação, com simulação e confirmação.

Ainda não há canal de instalação: a distribuição será só por código-fonte, com tag assinada (ver abaixo e o [ADR 0010](project/adr/0010-distribuicao-e-integridade.md)). A [spec inicial](project/SPEC.md) ainda usa o nome provisório "Universal". Escopo e fases em [`project/OVERVIEW.md`](project/OVERVIEW.md) e [`project/MVP.md`](project/MVP.md).

## Documentação

- [`project/INDEX.md`](project/INDEX.md): ponto de entrada.
- [`project/SPEC.md`](project/SPEC.md): visão de produto e especificação inicial.
- [`project/adr/`](project/adr/): registros de decisão de arquitetura.

## Verificar uma versão

Toda versão é uma tag Git assinada com esta chave SSH (ADR 0010):

```
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIC+jfuZXrHU6jIjb/FBOqbfPtC8nK61IBVv0cEoY4EtX nodus tag signing
SHA256:KNurUVWF0KhmHv42A3OYw6spZenOE0DrQCZ0jjNFnxI
```

Confira a chave também por um segundo canal, que não é este README: `https://api.github.com/users/masterlxz/ssh_signing_keys`.

```bash
echo "nodus $(cut -d' ' -f1,2 <<'EOF'
ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIC+jfuZXrHU6jIjb/FBOqbfPtC8nK61IBVv0cEoY4EtX
EOF
)" > allowed_signers
git -c gpg.format=ssh -c gpg.ssh.allowedSignersFile=allowed_signers tag -v <tag>
echo $?   # 0 = confiável
```

Vale o **código de saída**, não o texto: se a chave não estiver no `allowed_signers`, o git ainda imprime `Good "git" signature` (a assinatura confere) e sai com 1 (`No principal matched`).

## Licença

[MIT](LICENSE).
