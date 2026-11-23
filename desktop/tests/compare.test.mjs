// O contrato do `nodusctl compare --all --json` visto pela GUI (ADR 0026): objeto na raiz com `comparisons` (um por par de
// snapshots) e `unreadable` (snapshots que não deu para ler), parsing estrito. Cada lista (pacman, Flatpak) é "comparada"
// (`only_a`, `only_b`, `common`) ou "não comparável" (`lacks`); os dotfiles são "comparados" (`differ` com o estado de cada
// lado, ou nulo se o caminho não está na lista da máquina, e `same`) ou "não comparável". A fixture é inventada. Formato:
// `Comparison` em `crates/nodus-core/src/compare.rs` e o objeto montado em `compare_all_command` de `crates/nodus-cli/src/main.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parseCompare } from "../src/lib/compare.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/compare_all.json", import.meta.url), "utf8"));
const parse = (value) => parseCompare(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "CompareError" });
const rejectsWith = (value, chave) =>
  assert.throws(() => parse(value), (e) => e.name === "CompareError" && e.message.includes(chave), chave);

test("válido: um par com diferenças em tudo e outro com SO diferente e listas não comparáveis, mais um snapshot ilegível", () => {
  const c = parse(fixture());
  assert.equal(c.comparisons.length, 2);
  const [um, dois] = c.comparisons;
  assert.equal(um.a, "pc-a");
  assert.equal(um.b, "pc-b");
  assert.equal(um.os, null);
  assert.deepEqual(um.pacman, { kind: "compared", onlyA: ["vim"], onlyB: ["htop", "tmux"], common: 2 });
  assert.deepEqual(um.flatpak, { kind: "compared", onlyA: [], onlyB: ["org.b.B"], common: 1 });
  assert.deepEqual(um.dotfiles, {
    kind: "compared",
    differ: [
      { path: ".bashrc", a: "linked", b: "file" },
      { path: ".zshrc", a: "file", b: null },
    ],
    same: 7,
  });
  assert.deepEqual(dois.os, { a: { name: "Arch Linux", version: null }, b: { name: "Fedora Linux", version: "41" } });
  assert.deepEqual(dois.flatpak, { kind: "not_comparable", lacks: ["pc-c"] });
  assert.deepEqual(dois.dotfiles, { kind: "not_comparable", lacks: ["pc-c"] });
  assert.deepEqual(c.unreadable, [{ label: "quebrado", reason: "JSON inválido" }]);
});

test("sem nenhum par é válido (menos de dois snapshots legíveis não é erro)", () => {
  const c = parse({ comparisons: [], unreadable: [] });
  assert.deepEqual(c.comparisons, []);
  assert.deepEqual(c.unreadable, []);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data.novo = 1;
  data.comparisons[0].novo = 2;
  data.comparisons[0].pacman.novo = 3;
  data.comparisons[0].dotfiles.differ[0].novo = 4;
  assert.equal(parse(data).comparisons[0].a, "pc-a");
});

test("campo ausente é erro e o erro diz qual", () => {
  for (const chave of ["comparisons", "unreadable"]) {
    const data = fixture();
    delete data[chave];
    rejectsWith(data, chave);
  }
  for (const chave of ["a", "b", "os", "pacman", "flatpak", "dotfiles"]) {
    const data = fixture();
    delete data.comparisons[0][chave];
    rejectsWith(data, chave);
  }
  for (const chave of ["only_a", "only_b", "common"]) {
    const data = fixture();
    delete data.comparisons[0].pacman[chave];
    rejectsWith(data, chave);
  }
  for (const chave of ["differ", "same"]) {
    const data = fixture();
    delete data.comparisons[0].dotfiles[chave];
    rejectsWith(data, chave);
  }
  for (const chave of ["path", "a", "b"]) {
    const data = fixture();
    delete data.comparisons[0].dotfiles.differ[0][chave];
    rejectsWith(data, chave);
  }
  for (const lista of ["flatpak", "dotfiles"]) {
    const data = fixture();
    delete data.comparisons[1][lista].lacks;
    rejectsWith(data, "lacks");
  }
  for (const chave of ["name", "version"]) {
    const data = fixture();
    delete data.comparisons[1].os.a[chave];
    rejectsWith(data, chave);
  }
  for (const chave of ["label", "reason"]) {
    const data = fixture();
    delete data.unreadable[0][chave];
    rejectsWith(data, chave);
  }
});

test("o `status` de cada lista só pode ser `compared` ou `not_comparable`", () => {
  for (const valor of ["outro", "Compared", "", null, 1, undefined]) {
    for (const lista of ["pacman", "flatpak", "dotfiles"]) {
      const data = fixture();
      data.comparisons[0][lista].status = valor;
      rejects(data);
    }
  }
});

test("um `status` desconhecido é erro mesmo com os campos de outro status presentes (não adivinha qual era)", () => {
  // Um objeto com `lacks` e status desconhecido não pode virar "não comparável", nem um com `only_a` virar "comparado".
  for (const lista of ["pacman", "flatpak", "dotfiles"]) {
    const comLacks = fixture();
    comLacks.comparisons[0][lista] = { status: "outro", lacks: ["pc-a"] };
    rejects(comLacks);
    const completo = fixture();
    completo.comparisons[0][lista] = { status: "outro", lacks: ["pc-a"], only_a: [], only_b: [], common: 1, differ: [], same: 1 };
    rejects(completo);
  }
});

test("as listas de itens e de `lacks` precisam ser listas de texto", () => {
  for (const valor of ["htop", null, 1, {}, [1], [null], ["ok", 2], [["aninhada"]]]) {
    for (const chave of ["only_a", "only_b"]) {
      const data = fixture();
      data.comparisons[0].pacman[chave] = valor;
      rejects(data);
    }
    const lacks = fixture();
    lacks.comparisons[1].flatpak.lacks = valor;
    rejects(lacks);
  }
});

test("`common` e `same` precisam ser números inteiros, não negativos", () => {
  for (const valor of ["2", -1, 1.5, null, [], {}]) {
    const comum = fixture();
    comum.comparisons[0].pacman.common = valor;
    rejects(comum);
    const igual = fixture();
    igual.comparisons[0].dotfiles.same = valor;
    rejects(igual);
  }
});

test("o estado de um dotfile é `linked`, `file`, `missing`, `other` ou nulo (fora da lista)", () => {
  for (const estado of ["linked", "file", "missing", "other", null]) {
    const data = fixture();
    data.comparisons[0].dotfiles.differ[0].a = estado;
    assert.equal(parse(data).comparisons[0].dotfiles.differ[0].a, estado);
  }
  for (const valor of ["symlink", "Linked", "LINKED", "", 1, true, [], {}, undefined]) {
    for (const lado of ["a", "b"]) {
      const data = fixture();
      data.comparisons[0].dotfiles.differ[0][lado] = valor;
      rejects(data);
    }
  }
});

test("`differ` precisa ser uma lista de objetos com `path` em texto", () => {
  for (const valor of [null, "x", 1, {}, ["x"], [null], [{ path: 1, a: "file", b: "file" }]]) {
    const data = fixture();
    data.comparisons[0].dotfiles.differ = valor;
    rejects(data);
  }
});

test("`os` é nulo ou um objeto com `a` e `b`, e a versão é texto ou nula", () => {
  for (const valor of ["x", 1, true, [], { a: null, b: null }, { a: { name: "x", version: null } }]) {
    const data = fixture();
    data.comparisons[0].os = valor;
    rejects(data);
  }
  for (const valor of [1, true, [], {}]) {
    const data = fixture();
    data.comparisons[1].os.b.version = valor;
    rejects(data);
  }
});

test("os rótulos precisam ser texto", () => {
  for (const valor of [1, null, [], {}, true]) {
    for (const lado of ["a", "b"]) {
      const data = fixture();
      data.comparisons[0][lado] = valor;
      rejects(data);
    }
  }
});

test("a raiz precisa ser um objeto, e a saída precisa ser JSON", () => {
  for (const valor of [[], null, 1, "x", true]) rejects(valor);
  rejects("isto não é json");
  rejects("");
});
