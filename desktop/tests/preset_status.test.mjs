// O contrato do `nodusctl preset status <arquivo> --json` visto pela GUI (ADR 0026): objeto na raiz com `machines` (uma por
// snapshot) e `unreadable` (snapshots que não deu para ler), parsing estrito. Cada lista de uma máquina é "comparada" (com
// `missing`, `extra` e `in_sync`) ou "não comparável" (com `lacks`). A fixture é inventada. Formato: `MachineStatus` e
// `StatusList` em `crates/nodus-core/src/status.rs`, mais o objeto montado em `preset_status` de `crates/nodus-cli/src/main.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { exceptionSnippet, parsePresetStatus } from "../src/lib/presetStatus.ts";

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

// `exceptions` (ADR 0033, emenda 4): o trecho `[machines.<rótulo>]` que o `preset status` imprime no terminal. O formato vem de
// `exceptions_for` (`crates/nodus-core/src/status.rs`) e `status_json` (`crates/nodus-cli/src/render.rs`).

test("`exceptions` de cada máquina é lido: com e sem exceção", () => {
  const [a, b] = parse(fixture()).machines;
  assert.deepEqual(a.exceptions, {
    tableExists: false,
    add: { pacman: ["vim", "tmux"], flatpak: [] },
    skip: { pacman: ["htop"], flatpak: [] },
  });
  assert.deepEqual(b.exceptions, { tableExists: false, add: { pacman: [], flatpak: [] }, skip: { pacman: [], flatpak: [] } });
});

test("sem `exceptions` (ou com um campo dele faltando) é erro, e o erro diz qual: um nodusctl antigo não passa em silêncio", () => {
  const semExceptions = fixture();
  delete semExceptions.machines[0].exceptions;
  assert.throws(() => parse(semExceptions), (e) => e.name === "PresetStatusError" && e.message.includes("machines[0].exceptions"));
  for (const chave of ["table_exists", "add", "skip"]) {
    const data = fixture();
    delete data.machines[0].exceptions[chave];
    assert.throws(() => parse(data), (e) => e.name === "PresetStatusError" && e.message.includes(chave), chave);
  }
  for (const grupo of ["add", "skip"]) {
    for (const lista of ["pacman", "flatpak"]) {
      const data = fixture();
      delete data.machines[0].exceptions[grupo][lista];
      assert.throws(() => parse(data), (e) => e.name === "PresetStatusError" && e.message.includes(lista), `${grupo}.${lista}`);
    }
  }
});

test("`exceptions` com tipo errado é erro", () => {
  for (const valor of ["sim", 1, null, [], {}]) {
    const data = fixture();
    data.machines[0].exceptions.table_exists = valor;
    rejects(data);
  }
  for (const valor of ["git", null, 1, {}, [1], ["ok", null]]) {
    for (const [grupo, lista] of [["add", "pacman"], ["add", "flatpak"], ["skip", "pacman"], ["skip", "flatpak"]]) {
      const data = fixture();
      data.machines[0].exceptions[grupo][lista] = valor;
      rejects(data);
    }
  }
  for (const valor of ["x", null, [], 1]) {
    const data = fixture();
    data.machines[0].exceptions = valor;
    rejects(data);
  }
});

const semExcecao = { tableExists: false, add: { pacman: [], flatpak: [] }, skip: { pacman: [], flatpak: [] } };

test("trecho sem a tabela da máquina: a tabela inteira, com as quatro chaves na ordem add e depois skip", () => {
  // Mesmo texto que o `status_com_pacman_e_flatpak_poe_as_quatro_chaves_na_ordem_add_depois_skip` do render.rs fixa no terminal.
  const trecho = exceptionSnippet("notebook", {
    tableExists: false,
    add: { pacman: ["steam", "sl"], flatpak: ["org.B.B"] },
    skip: { pacman: ["vim"], flatpak: ["org.A.A"] },
  });
  assert.equal(trecho.intro, "Para registrar como exceção desta máquina, cole no preset (nada foi alterado):");
  assert.equal(
    trecho.body,
    '[machines.notebook]\nadd_pacman = ["steam", "sl"]\nadd_flatpak = ["org.B.B"]\nskip_pacman = ["vim"]\nskip_flatpak = ["org.A.A"]',
  );
});

test("trecho só com uma lista: não escreve as chaves vazias", () => {
  const trecho = exceptionSnippet("notebook", { ...semExcecao, skip: { pacman: ["vim"], flatpak: [] } });
  assert.equal(trecho.body, '[machines.notebook]\nskip_pacman = ["vim"]');
});

test("trecho com a tabela da máquina já existente: diz em qual lista acrescentar, sem repetir a tabela (seria TOML inválido)", () => {
  // Mesmo texto que o `status_com_a_tabela_ja_existente_manda_acrescentar_na_lista_sem_repetir_a_tabela` do render.rs.
  const trecho = exceptionSnippet("notebook", { tableExists: true, add: { pacman: ["steam"], flatpak: [] }, skip: { pacman: [], flatpak: [] } });
  assert.equal(trecho.intro, "A tabela [machines.notebook] já existe. Para registrar a exceção, acrescente (nada foi alterado):");
  assert.equal(trecho.body, 'em `add_pacman = [...]`:\n  "steam",');
  assert.doesNotMatch(trecho.body, /\[machines\./);
});

test("sem nenhuma exceção não há trecho", () => {
  assert.equal(exceptionSnippet("notebook", semExcecao), null);
  assert.equal(exceptionSnippet("notebook", { ...semExcecao, tableExists: true }), null);
});
