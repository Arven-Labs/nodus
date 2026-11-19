// O contrato do `nodusctl preset suggest <arquivo> --json` visto pela GUI (ADR 0026): objeto na raiz, parsing estrito, quatro
// listas de texto (`pacman_extra`, `foreign_extra`, `flatpak_extra`, `ignored`) e `flatpak_installed` booleano. A fixture é
// inventada. Formato: `Suggestions` em `crates/nodus-core/src/preset.rs` mais `ignored`, de `suggestions_json` em
// `crates/nodus-cli/src/render.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parsePresetSuggest } from "../src/lib/presetSuggest.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/preset_suggest.json", import.meta.url), "utf8"));
const parse = (value) => parsePresetSuggest(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "PresetSuggestError" });

test("sugestões válidas: o que está fora do preset, o que é de fora dos repositórios e o que está escondido", () => {
  const s = parse(fixture());
  assert.deepEqual(s.pacmanExtra, ["neovim", "tmux"]);
  assert.deepEqual(s.foreignExtra, ["brave-bin"]);
  assert.deepEqual(s.flatpakExtra, ["org.mozilla.firefox"]);
  assert.equal(s.flatpakInstalled, true);
  assert.deepEqual(s.ignored, ["cmatrix", "sl"]);
});

test("sem sugestões (tudo já está no preset) é válido", () => {
  const s = parse({ pacman_extra: [], foreign_extra: [], flatpak_extra: [], flatpak_installed: false, ignored: [] });
  assert.deepEqual(s.pacmanExtra, []);
  assert.deepEqual(s.ignored, []);
  assert.equal(s.flatpakInstalled, false);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data.novo = 1;
  assert.deepEqual(parse(data).pacmanExtra, ["neovim", "tmux"]);
});

test("campo ausente é erro e o erro diz qual", () => {
  for (const chave of Object.keys(fixture())) {
    const data = fixture();
    delete data[chave];
    assert.throws(() => parse(data), (e) => e.name === "PresetSuggestError" && e.message.includes(chave), chave);
  }
});

test("as listas precisam ser listas de texto", () => {
  for (const chave of ["pacman_extra", "foreign_extra", "flatpak_extra", "ignored"]) {
    for (const valor of ["htop", null, 1, {}, [1], [null], ["ok", 2], [["aninhada"]]]) {
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
