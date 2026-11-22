// O contrato do `nodusctl preset status <arquivo> --json` visto pela GUI (ADR 0026): objeto na raiz com `machines` (uma por
// snapshot) e `unreadable` (snapshots que não deu para ler), parsing estrito. Cada lista de uma máquina é "comparada" (com
// `missing`, `extra` e `in_sync`) ou "não comparável" (com `lacks`). A fixture é inventada. Formato: `MachineStatus` e
// `StatusList` em `crates/nodus-core/src/status.rs`, mais o objeto montado em `preset_status` de `crates/nodus-cli/src/main.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parsePresetStatus } from "../src/lib/presetStatus.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/preset_status.json", import.meta.url), "utf8"));
const parse = (value) => parsePresetStatus(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "PresetStatusError" });

test("status válido: uma máquina com diferenças, outra em dia com o Flatpak não comparável, e um snapshot ilegível", () => {
  const s = parse(fixture());
  assert.equal(s.machines.length, 2);
  const [a, b] = s.machines;
  assert.equal(a.machine, "archlinux");
  assert.deepEqual(a.os, { name: "Arch Linux", version: null });
  assert.deepEqual(a.pacman, { kind: "compared", missing: ["htop"], extra: ["vim", "tmux"], inSync: 2 });
  assert.deepEqual(a.flatpak, { kind: "compared", missing: [], extra: [], inSync: 1 });
  assert.equal(b.machine, "pc-fake");
  assert.equal(b.os.version, "rolling");
  assert.deepEqual(b.flatpak, { kind: "not_comparable", lacks: ["pc-fake"] });
  assert.deepEqual(s.unreadable, [{ label: "quebrado", reason: "JSON inválido" }]);
});

test("sem nenhum snapshot é válido (listas vazias)", () => {
  const s = parse({ machines: [], unreadable: [] });
  assert.deepEqual(s.machines, []);
  assert.deepEqual(s.unreadable, []);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data.novo = 1;
  data.machines[0].novo = 2;
  data.machines[0].pacman.novo = 3;
  assert.equal(parse(data).machines[0].machine, "archlinux");
});

test("campo ausente é erro e o erro diz qual", () => {
  for (const chave of ["machines", "unreadable"]) {
    const data = fixture();
    delete data[chave];
    assert.throws(() => parse(data), (e) => e.name === "PresetStatusError" && e.message.includes(chave), chave);
  }
  for (const chave of ["machine", "os", "pacman", "flatpak"]) {
    const data = fixture();
    delete data.machines[0][chave];
    assert.throws(() => parse(data), (e) => e.name === "PresetStatusError" && e.message.includes(chave), chave);
  }
  for (const chave of ["missing", "extra", "in_sync"]) {
    const data = fixture();
    delete data.machines[0].pacman[chave];
    assert.throws(() => parse(data), (e) => e.name === "PresetStatusError" && e.message.includes(chave), chave);
  }
  const semLacks = fixture();
  delete semLacks.machines[1].flatpak.lacks;
  assert.throws(() => parse(semLacks), (e) => e.name === "PresetStatusError" && e.message.includes("lacks"));
  for (const chave of ["label", "reason"]) {
    const data = fixture();
    delete data.unreadable[0][chave];
    assert.throws(() => parse(data), (e) => e.name === "PresetStatusError" && e.message.includes(chave), chave);
  }
});

test("o `status` de cada lista só pode ser `compared` ou `not_comparable`", () => {
  for (const valor of ["outro", "Compared", "", null, 1, undefined]) {
    const data = fixture();
    data.machines[0].pacman.status = valor;
    rejects(data);
  }
});

test("as listas de itens precisam ser listas de texto", () => {
  for (const valor of ["htop", null, 1, {}, [1], [null], ["ok", 2], [["aninhada"]]]) {
    for (const chave of ["missing", "extra"]) {
      const data = fixture();
      data.machines[0].pacman[chave] = valor;
      rejects(data);
    }
    const naoComparavel = fixture();
    naoComparavel.machines[1].flatpak.lacks = valor;
    rejects(naoComparavel);
  }
});

test("`in_sync` precisa ser um número inteiro, não negativo", () => {
  for (const valor of ["2", -1, 1.5, null, [], {}]) {
    const data = fixture();
    data.machines[0].pacman.in_sync = valor;
    rejects(data);
  }
});

test("`os.version` é texto ou nulo", () => {
  for (const valor of [1, true, [], {}]) {
    const data = fixture();
    data.machines[0].os.version = valor;
    rejects(data);
  }
});

test("a raiz precisa ser um objeto, e a saída precisa ser JSON", () => {
  for (const valor of [[], null, 1, "x", true]) rejects(valor);
  rejects("isto não é json");
  rejects("");
});
