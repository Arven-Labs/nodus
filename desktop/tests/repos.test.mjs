// O contrato do `nodusctl repos --json` visto pela GUI (ADR 0026): array na raiz, parsing estrito, e `fetch` só `null`
// (a GUI nunca pede `--fetch`). A fixture é inventada. Formato: `crates/nodus-core/src/repos.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parseRepos } from "../src/lib/repos.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/repos.json", import.meta.url), "utf8"));
const parse = (value) => parseRepos(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "ReposError" });

test("repositórios válidos: nome, branch, ahead/behind, upstream removido e erro", () => {
  const repos = parse(fixture());
  assert.deepEqual(repos.map((r) => r.name), ["alpha", "beta", "gamma", "delta"]);
  assert.equal(repos[0].branch, "main");
  assert.equal(repos[0].ahead, 2);
  assert.equal(repos[0].behind, 0);
  assert.equal(repos[0].upstreamGone, false);
  assert.equal(repos[1].upstream, null);
  assert.equal(repos[1].ahead, null, "sem upstream: null, não 0");
  assert.equal(repos[2].upstreamGone, true);
  assert.equal(repos[3].branch, null);
  assert.equal(repos[3].error, "git terminou com status 128");
});

test("lista vazia é válida", () => {
  assert.deepEqual(parse([]), []);
});

test("campos desconhecidos são ignorados", () => {
  const data = fixture();
  data[0].novo = { qualquer: "coisa" };
  assert.equal(parse(data)[0].name, "alpha");
});

test("falta de campo obrigatório é erro, com o caminho do campo", () => {
  for (const campo of ["name", "path", "branch", "ahead", "upstream_gone", "error", "fetch"]) {
    const data = fixture();
    delete data[0][campo];
    assert.throws(() => parse(data), (e) => e.name === "ReposError" && e.message.includes(`repos[0].${campo}`), campo);
  }
});

test("tipo errado é erro", () => {
  const casos = [
    (d) => (d[0].name = 1),
    (d) => (d[0].branch = 5),
    (d) => (d[0].ahead = -1),
    (d) => (d[0].ahead = 1.5),
    (d) => (d[0].ahead = "2"),
    (d) => (d[0].behind = -3),
    (d) => (d[0].upstream_gone = "false"),
    (d) => (d[0].upstream_gone = null),
    (d) => (d[0].error = 7),
    (d) => (d[1] = "texto"),
    (d) => (d[1] = null),
  ];
  for (const quebra of casos) {
    const data = fixture();
    quebra(data);
    rejects(data);
  }
});

test("fetch só pode ser null: a GUI nunca pede --fetch", () => {
  for (const valor of ["done", { failed: "sem rede" }, true, 0]) {
    const data = fixture();
    data[0].fetch = valor;
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
