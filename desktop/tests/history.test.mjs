// O contrato do `nodusctl history --json` visto pela GUI (ADR 0026): array na raiz, parsing estrito, `action`, `manager` e
// `result.status` só os valores conhecidos (outro valor quer dizer que o contrato mudou). A fixture é inventada. Formato:
// `crates/nodus-core/src/history.rs` (`HistoryEntry`). O número da entrada (a partir de 1) é a posição no array.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { canUndo, formatUtc, parseHistory } from "../src/lib/history.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/history.json", import.meta.url), "utf8"));
const parse = (value) => parseHistory(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "HistoryError" });

test("histórico válido: ação, gerenciador, resultado, reversão e pasta-fonte", () => {
  const h = parse(fixture());
  assert.deepEqual(h.map((e) => e.action), ["install", "install", "remove", "adopt", "link"]);
  assert.deepEqual(h.map((e) => e.manager), ["pacman", "flatpak", "pacman", "dotfiles", "dotfiles"]);
  assert.deepEqual(h[0].requested, ["sl", "tree"]);
  assert.deepEqual(h[0].command.slice(0, 2), ["sudo", "pacman"]);
  assert.deepEqual(h[0].result, { status: "ok" });
  assert.deepEqual(h[1].result, { status: "failed", code: 1 });
  assert.deepEqual(h[3].result, { status: "failed", code: null });
  assert.equal(h[2].undoes, 1);
  assert.equal(h[0].undoes, null, "ausente é null");
  assert.equal(h[3].dir, "/home/exemplo/dotfiles");
  assert.equal(h[0].dir, null);
  assert.equal(h[0].atUnix, 1700000000);
});

test("lista vazia é válida", () => {
  assert.deepEqual(parse([]), []);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data[0].novo = 1;
  assert.equal(parse(data)[0].action, "install");
});

test("`action`, `manager` e `result.status` desconhecidos são erro (o contrato mudou)", () => {
  for (const [campo, valor] of [["action", "purge"], ["action", null], ["manager", "aur"], ["manager", ""]]) {
    const data = fixture();
    data[0][campo] = valor;
    assert.throws(() => parse(data), (e) => e.name === "HistoryError" && e.message.includes(campo), `${campo}=${valor}`);
  }
  const data = fixture();
  data[0].result = { status: "pending" };
  assert.throws(() => parse(data), (e) => e.name === "HistoryError" && e.message.includes("status"));
});

test("`failed` exige `code` (número inteiro ou null)", () => {
  for (const valor of ["1", 1.5, undefined]) {
    const data = fixture();
    data[1].result = valor === undefined ? { status: "failed" } : { status: "failed", code: valor };
    rejects(data);
  }
});

test("`at_unix` e `undoes` precisam ser contagens (inteiro, não negativo)", () => {
  for (const valor of [-1, 1.5, "1", null]) {
    const data = fixture();
    data[0].at_unix = valor;
    rejects(data);
  }
  for (const valor of [-1, 1.5, "1"]) {
    const data = fixture();
    data[2].undoes = valor;
    rejects(data);
  }
});

test("campo ausente ou de tipo errado é erro", () => {
  for (const campo of ["version", "at_unix", "command", "requested", "installed", "still_missing", "result", "action", "manager"]) {
    const data = fixture();
    delete data[0][campo];
    rejects(data);
  }
  const comandoComNumero = fixture();
  comandoComNumero[0].command = ["sudo", 1];
  rejects(comandoComNumero);
  const dirNumero = fixture();
  dirNumero[3].dir = 7;
  rejects(dirNumero);
});

test("raiz que não é lista, ou texto que não é JSON, é erro", () => {
  rejects({});
  rejects("null");
  rejects("isto não é json");
});

test("formatUtc: mesma forma do `nodusctl history` (UTC, sem fuso local)", () => {
  assert.equal(formatUtc(1700000000), "2023-11-14 22:13:20 UTC");
  assert.equal(formatUtc(0), "1970-01-01 00:00:00 UTC");
});

// `canUndo` só decide mostrar o botão; quem recusa de verdade é a CLI. Entradas mínimas inventadas.
const entry = (over) => ({
  version: 3,
  atUnix: 1700000000,
  command: [],
  requested: ["org.exemplo.App"],
  installed: ["org.exemplo.App"],
  stillMissing: [],
  result: { status: "ok" },
  action: "install",
  undoes: null,
  manager: "flatpak",
  dir: null,
  ...over,
});
const flatpakRemove = (undoes, result = { status: "ok" }) => entry({ action: "remove", undoes, result, installed: [] });

test("canUndo: instalação Flatpak ainda não desfeita tem o botão", () => {
  assert.equal(canUndo([entry({})], 1), true);
  assert.equal(canUndo([entry({ result: { status: "failed", code: 1 } })], 1), true, "como no pacman: a CLI é quem recusa");
});

test("canUndo: Flatpak já desfeito com sucesso perde o botão; reversão que falhou deixa tentar de novo", () => {
  assert.equal(canUndo([entry({}), flatpakRemove(1)], 1), false);
  assert.equal(canUndo([entry({}), flatpakRemove(1, { status: "failed", code: 1 })], 1), true);
});

test("canUndo: reversão Flatpak nunca tem o botão; as outras regras seguem", () => {
  assert.equal(canUndo([entry({}), flatpakRemove(1)], 2), false);
  assert.equal(canUndo([entry({ manager: "pacman" })], 1), true);
  assert.equal(canUndo([entry({ manager: "dotfiles", action: "adopt" })], 1), true);
  assert.equal(canUndo([entry({ manager: "dotfiles", action: "remove", undoes: 1 })], 1), false);
  assert.equal(canUndo([entry({})], 2), false, "entrada inexistente");
});
