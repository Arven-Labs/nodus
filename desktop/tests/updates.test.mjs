// O contrato do `nodusctl updates --json` visto pela GUI (ADR 0026): objeto na raiz, parsing estrito, `pacman_source` só um dos
// dois valores conhecidos e `flatpak` `null` quando o Flatpak não está instalado. A fixture é inventada. Formato:
// `crates/nodus-core/src/updates.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parseUpdates } from "../src/lib/updates.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/updates.json", import.meta.url), "utf8"));
const parse = (value) => parseUpdates(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "UpdatesError" });

test("atualizações válidas: pacman, origem e Flatpak", () => {
  const u = parse(fixture());
  assert.equal(u.pacmanSource, "checkupdates");
  assert.deepEqual(u.pacman.map((p) => p.name), ["alpha-lib", "beta-tool"]);
  assert.equal(u.pacman[1].oldVersion, "2:3.4-2");
  assert.equal(u.pacman[1].newVersion, "2:3.5-1");
  assert.deepEqual(u.flatpak.map((f) => f.application), ["org.exemplo.App", "org.exemplo.SemVersao"]);
  assert.equal(u.flatpak[0].version, "4.2");
  assert.equal(u.flatpak[0].branch, "stable");
  assert.equal(u.flatpak[1].version, null);
});

test("`pacman-qu` é aceito (origem que pode estar desatualizada)", () => {
  const data = fixture();
  data.pacman_source = "pacman-qu";
  assert.equal(parse(data).pacmanSource, "pacman-qu");
});

test("Flatpak não instalado (`null`) é diferente de nenhuma atualização (`[]`)", () => {
  const sem = fixture();
  sem.flatpak = null;
  assert.equal(parse(sem).flatpak, null);
  const vazio = fixture();
  vazio.flatpak = [];
  assert.deepEqual(parse(vazio).flatpak, []);
});

test("sem nenhuma atualização é válido", () => {
  const data = fixture();
  data.pacman = [];
  assert.deepEqual(parse(data).pacman, []);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data.novo = { qualquer: "coisa" };
  data.pacman[0].extra = 1;
  assert.equal(parse(data).pacman[0].name, "alpha-lib");
});

test("`pacman_source` desconhecido é erro (o contrato mudou)", () => {
  for (const valor of ["aur", "", "Checkupdates", null, 1]) {
    const data = fixture();
    data.pacman_source = valor;
    assert.throws(() => parse(data), (e) => e.name === "UpdatesError" && e.message.includes("pacman_source"), String(valor));
  }
});

test("falta de campo obrigatório é erro, com o caminho do campo", () => {
  const casos = [
    ["pacman_source", (d) => delete d.pacman_source],
    ["pacman", (d) => delete d.pacman],
    ["flatpak", (d) => delete d.flatpak],
    ["pacman[0].name", (d) => delete d.pacman[0].name],
    ["pacman[0].old_version", (d) => delete d.pacman[0].old_version],
    ["pacman[0].new_version", (d) => delete d.pacman[0].new_version],
    ["flatpak[0].application", (d) => delete d.flatpak[0].application],
    ["flatpak[0].version", (d) => delete d.flatpak[0].version],
    ["flatpak[0].branch", (d) => delete d.flatpak[0].branch],
  ];
  for (const [caminho, quebra] of casos) {
    const data = fixture();
    quebra(data);
    assert.throws(() => parse(data), (e) => e.name === "UpdatesError" && e.message.includes(caminho), caminho);
  }
});

test("tipo errado é erro", () => {
  const casos = [
    (d) => (d.pacman = null),
    (d) => (d.pacman = {}),
    (d) => (d.flatpak = {}),
    (d) => (d.flatpak = "nenhum"),
    (d) => (d.pacman[0].name = 1),
    (d) => (d.pacman[0].old_version = null),
    (d) => (d.pacman[0].new_version = 2),
    (d) => (d.pacman[1] = "texto"),
    (d) => (d.pacman[1] = null),
    (d) => (d.flatpak[0].application = null),
    (d) => (d.flatpak[0].version = 4.2),
    (d) => (d.flatpak[0].branch = null),
    (d) => (d.flatpak[1] = []),
  ];
  for (const quebra of casos) {
    const data = fixture();
    quebra(data);
    rejects(data);
  }
});

test("raiz que não é objeto, ou texto que não é JSON, é erro", () => {
  rejects("not json");
  rejects("");
  rejects("[]");
  rejects("null");
  rejects("42");
});
