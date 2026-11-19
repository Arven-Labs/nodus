// O contrato do resultado de `run_action` visto pela GUI (ADR 0027): objeto na raiz, parsing estrito, `launched` booleano e
// `command` em texto (o comando exato, só para mostrar). Diferente das leituras, o `invoke` já devolve um objeto, não JSON em
// texto. Formato: `Outcome` em `src-tauri/src/actions.rs`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { parseActionOutcome } from "../src/lib/action.ts";

const valido = () => ({ launched: true, command: "/usr/bin/nodusctl preset apply /tmp/p.toml" });
const rejects = (value) => assert.throws(() => parseActionOutcome(value), { name: "ActionError" });

test("resultado válido: terminal lançado", () => {
  const o = parseActionOutcome(valido());
  assert.equal(o.launched, true);
  assert.equal(o.command, "/usr/bin/nodusctl preset apply /tmp/p.toml");
});

test("resultado válido: sem terminal (o comando para colar)", () => {
  const o = parseActionOutcome({ launched: false, command: "nodusctl preset apply '/tmp/minha pasta/p.toml'" });
  assert.equal(o.launched, false);
  assert.match(o.command, /minha pasta/);
});

test("campos desconhecidos são ignorados", () => {
  const data = { ...valido(), novo: 1 };
  assert.equal(parseActionOutcome(data).launched, true);
});

test("campo ausente é erro e o erro diz qual", () => {
  for (const chave of Object.keys(valido())) {
    const data = valido();
    delete data[chave];
    assert.throws(() => parseActionOutcome(data), (e) => e.name === "ActionError" && e.message.includes(chave), chave);
  }
});

test("`launched` precisa ser booleano e `command` precisa ser texto", () => {
  for (const valor of ["true", 1, 0, null, []]) rejects({ ...valido(), launched: valor });
  for (const valor of [1, null, [], {}, true]) rejects({ ...valido(), command: valor });
});

test("a raiz precisa ser um objeto", () => {
  for (const valor of [[], null, 1, "x", true, undefined]) rejects(valor);
});
