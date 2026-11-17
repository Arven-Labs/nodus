// O contrato do `nodusctl dotfiles --json` visto pela GUI (ADR 0026): array na raiz, `kind` decide os campos, `kind`
// desconhecido é erro. A fixture é inventada. Formato: `crates/nodus-core/src/dotfiles.rs` (`Entry` com `Kind` achatado).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parseDotfiles } from "../src/lib/dotfiles.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/dotfiles.json", import.meta.url), "utf8"));
const parse = (value) => parseDotfiles(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "DotfilesError" });

test("entradas válidas: arquivo, symlink, ausente, pasta e outro, na ordem da CLI", () => {
  const entradas = parse(fixture());
  assert.deepEqual(entradas.map((e) => e.path), [".bashrc", ".gitconfig", ".profile", ".vimrc", ".config/git", ".zshrc"]);
  assert.deepEqual(entradas.map((e) => e.kind), ["file", "symlink", "symlink", "missing", "dir", "other"]);
  assert.equal(entradas[0].size, 1234);
  assert.equal(entradas[1].target, "/dotfiles-teste/.gitconfig");
  assert.equal(entradas[1].outsideHome, true);
  assert.equal(entradas[2].outsideHome, false);
});

test("lista vazia é válida", () => {
  assert.deepEqual(parse([]), []);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data[0].novo = 1;
  assert.equal(parse(data)[0].path, ".bashrc");
});

test("kind desconhecido é erro, citando o campo", () => {
  for (const kind of ["weird", "File", "", "fifo", null, 3]) {
    const data = fixture();
    data[0].kind = kind;
    assert.throws(() => parse(data), (e) => e.name === "DotfilesError" && e.message.includes("dotfiles[0].kind"), String(kind));
  }
});

test("falta de campo obrigatório é erro, com o caminho do campo", () => {
  const casos = [
    [0, "path"],
    [0, "kind"],
    [0, "size"],
    [1, "target"],
    [1, "outside_home"],
  ];
  for (const [i, campo] of casos) {
    const data = fixture();
    delete data[i][campo];
    assert.throws(() => parse(data), (e) => e.name === "DotfilesError" && e.message.includes(`dotfiles[${i}].${campo}`), campo);
  }
});

test("tipo errado é erro", () => {
  const casos = [
    (d) => (d[0].path = 1),
    (d) => (d[0].size = -1),
    (d) => (d[0].size = 1.5),
    (d) => (d[0].size = "12"),
    (d) => (d[1].target = null),
    (d) => (d[1].outside_home = "true"),
    (d) => (d[1].outside_home = null),
    (d) => (d[3] = "texto"),
    (d) => (d[3] = null),
  ];
  for (const quebra of casos) {
    const data = fixture();
    quebra(data);
    rejects(data);
  }
});

test("raiz que não é lista, ou texto que não é JSON, é erro", () => {
  rejects("not json");
  rejects("");
  rejects("{}");
  rejects("null");
  rejects("42");
});
