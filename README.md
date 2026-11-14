# nodus

Plataforma open source e extensível para conhecer, configurar, padronizar, monitorar e administrar computadores, servidores, VPS, ambientes de desenvolvimento e aplicações em um único lugar. Local-first, self-hostable e independente de fabricante.

**Status:** pré-projeto. Só a [spec inicial](project/SPEC.md) existe (ela ainda usa o nome provisório "Universal"); stack e fases serão decididas depois. Estado em [`project/OVERVIEW.md`](project/OVERVIEW.md).

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
