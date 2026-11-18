// O contrato do `nodusctl preset plan <arquivo> --json` visto pela GUI (ADR 0026): objeto na raiz, parsing estrito, listas de
// texto, contagens inteiras não negativas e `flatpak_installed` booleano. A fixture é inventada. Formato: `Plan` em
// `crates/nodus-core/src/preset.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parsePresetPlan } from "../src/lib/preset.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/preset_plan.json", import.meta.url), "utf8"));
const parse = (value) => parsePresetPlan(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "PresetPlanError" });

test("plano válido: o que falta, o que já está e o que o pacman não acha", () => {
  const p = parse(fixture());
  assert.deepEqual(p.pacmanMissing, ["htop", "ripgrep"]);
  assert.equal(p.pacmanPresent, 150);
  assert.deepEqual(p.pacmanUnavailable, ["brave-bin"]);
  assert.deepEqual(p.flatpakMissing, ["org.gnome.Calculator"]);
  assert.equal(p.flatpakPresent, 4);
  assert.equal(p.flatpakInstalled, true);
});

test("plano vazio (a máquina já está como o preset) é válido", () => {
  const p = parse({
    pacman_missing: [],
    pacman_present: 0,
    pacman_unavailable: [],
    flatpak_missing: [],
    flatpak_present: 0,
    flatpak_installed: false,
  });
  assert.deepEqual(p.pacmanMissing, []);
  assert.equal(p.flatpakInstalled, false);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data.novo = 1;
  assert.deepEqual(parse(data).pacmanMissing, ["htop", "ripgrep"]);
});

test("campo ausente é erro e o erro diz qual", () => {
  for (const chave of Object.keys(fixture())) {
    const data = fixture();
    delete data[chave];
    assert.throws(() => parse(data), (e) => e.name === "PresetPlanError" && e.message.includes(chave), chave);
  }
});

test("as listas precisam ser listas de texto", () => {
  for (const chave of ["pacman_missing", "pacman_unavailable", "flatpak_missing"]) {
    for (const valor of ["htop", null, 1, {}, [1], [null], ["ok", 2], [["aninhada"]]]) {
      const data = fixture();
      data[chave] = valor;
      rejects(data);
    }
  }
});

test("as contagens precisam ser inteiros não negativos", () => {
  for (const chave of ["pacman_present", "flatpak_present"]) {
    for (const valor of [-1, 1.5, "3", null, 2 ** 60]) {
      const data = fixture();
      data[chave] = valor;
      rejects(data);
    }
  }
});

test("`flatpak_installed` precisa ser booleano", () => {
  for (const valor of ["true", 1, 0, null, []]) {
    const data = fixture();
    data.flatpak_installed = valor;
    rejects(data);
  }
});

test("a raiz precisa ser um objeto, e a saída precisa ser JSON", () => {
  for (const valor of [[], null, 1, "x", true]) rejects(valor);
  rejects("isto não é json");
  rejects("");
});
