// O contrato do `nodusctl workspace --json` visto pela GUI (ADR 0026): objeto na raiz, parsing estrito, `chosen` `null` quando
// nada foi escolhido, `source` só um dos quatro valores conhecidos. A fixture é inventada. Formato:
// `crates/nodus-cli/src/workspace.rs` (`report_json`).

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parseWorkspace } from "../src/lib/workspace.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/workspace.json", import.meta.url), "utf8"));
const parse = (value) => parseWorkspace(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "WorkspaceError" });

test("workspace válido: escolhido, origem e candidatos", () => {
  const w = parse(fixture());
  assert.equal(w.chosen.path, "/home/exemplo/ws");
  assert.equal(w.chosen.source, "detected");
  assert.equal(w.chosen.repoCount, 3);
  assert.deepEqual(w.candidates.map((c) => c.path), ["/home/exemplo/ws", "/home/exemplo/Projects"]);
  assert.equal(w.candidates[1].repoCount, 0);
});

test("nada escolhido (`chosen` null) é válido, com ou sem candidatos", () => {
  assert.equal(parse({ chosen: null, candidates: [] }).chosen, null);
  const data = fixture();
  data.chosen = null;
  assert.equal(parse(data).chosen, null);
  assert.equal(parse(data).candidates.length, 2);
});

test("as quatro origens são aceitas", () => {
  for (const source of ["informed", "environment", "stored", "detected"]) {
    const data = fixture();
    data.chosen.source = source;
    assert.equal(parse(data).chosen.source, source);
  }
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data.novo = 1;
  data.chosen.extra = "x";
  assert.equal(parse(data).chosen.path, "/home/exemplo/ws");
});

test("`source` desconhecida é erro (o contrato mudou)", () => {
  for (const valor of ["config", "", "Detected", null, 1]) {
    const data = fixture();
    data.chosen.source = valor;
    assert.throws(() => parse(data), (e) => e.name === "WorkspaceError" && e.message.includes("source"), String(valor));
  }
});

test("`repo_count` precisa ser contagem (inteiro, não negativo)", () => {
  for (const valor of [-1, 1.5, "3", null]) {
    const data = fixture();
    data.chosen.repo_count = valor;
    rejects(data);
    const outro = fixture();
    outro.candidates[0].repo_count = valor;
    rejects(outro);
  }
});

test("campo ausente ou de tipo errado é erro", () => {
  for (const campo of ["chosen", "candidates"]) {
    const data = fixture();
    delete data[campo];
    rejects(data);
  }
  const semPath = fixture();
  delete semPath.chosen.path;
  rejects(semPath);
  const pathNumero = fixture();
  pathNumero.candidates[0].path = 7;
  rejects(pathNumero);
  const candidatosObjeto = fixture();
  candidatosObjeto.candidates = {};
  rejects(candidatosObjeto);
});

test("raiz que não é objeto, ou texto que não é JSON, é erro", () => {
  rejects([]);
  rejects("null");
  rejects("isto não é json");
});
