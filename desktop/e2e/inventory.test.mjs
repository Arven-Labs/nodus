// A tela de inventário num navegador, com o backend de mentira (ADR 0026). Prova a tela e o que ela pede ao backend:
// só `read_nodusctl` com `{ command: "inventory" }`, nada que escreva. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const fixtureText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const fixture = () => JSON.parse(fixtureText);

let app;
let browser;
before(async () => {
  app = await serveDist();
  browser = await launchBrowser();
});
after(async () => {
  await browser?.close();
  await app?.stop();
});

const reads = (calls) => calls.filter((c) => c.cmd === "read_nodusctl");

test("inventário válido: mostra sistema, kernel, CPU, memória e pacotes", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: { read_nodusctl: fixtureText } });
  await page.getByTestId("system").waitFor();
  assert.match(await page.getByTestId("system").innerText(), /Arch Linux/);
  assert.match(await page.getByTestId("kernel").innerText(), /9\.9\.9-test/);
  assert.match(await page.getByTestId("cpu").innerText(), /Fictional CPU 9000/);
  assert.match(await page.getByTestId("memory").innerText(), /16 GiB/);
  assert.match(await page.getByTestId("packages-count").innerText(), /\b2\b/);
  assert.deepEqual(reads(calls), [{ cmd: "read_nodusctl", args: { command: "inventory" } }]);
  assert.deepEqual(errors, []);
  await close();
});

test("a tela só pede inventário: nenhum comando que escreve", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: { read_nodusctl: fixtureText } });
  await page.getByTestId("system").waitFor();
  const comandos = calls.map((c) => c.cmd);
  assert.deepEqual([...new Set(comandos)], ["read_nodusctl"]);
  await close();
});

test("versão incompatível: bloqueia, explica e não mostra inventário", async () => {
  const erro = { kind: "version_mismatch", message: "nodusctl 0.2.0 não é compatível com esta interface (0.1.x)" };
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: { read_nodusctl: { __error: erro } } });
  await page.getByTestId("versao-incompativel").waitFor();
  assert.match(await page.getByTestId("versao-incompativel").innerText(), /0\.2\.0/);
  assert.equal(await page.getByTestId("system").count(), 0);
  assert.equal(reads(calls).length, 1);
  assert.deepEqual(errors, []);
  await close();
});

test("nodusctl não encontrado: mensagem de erro, sem travar", async () => {
  const erro = { kind: "not_found", message: "nodusctl não encontrado no PATH" };
  const { page, errors, close } = await openApp(browser, app, { handlers: { read_nodusctl: { __error: erro } } });
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /não encontrado/);
  assert.equal(await page.getByTestId("system").count(), 0);
  assert.deepEqual(errors, []);
  await close();
});

test("JSON inesperado: erro visível com o campo, sem travar", async () => {
  const quebrado = fixture();
  delete quebrado.hardware;
  const { page, errors, close } = await openApp(browser, app, { handlers: { read_nodusctl: JSON.stringify(quebrado) } });
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /hardware/);
  assert.equal(await page.getByTestId("system").count(), 0);
  assert.deepEqual(errors, []);
  await close();
});

test("nomes hostis viram texto: nada é executado nem vira elemento", async () => {
  const hostil = fixture();
  const payload = '<img src=x onerror="window.__pwned=1">';
  hostil.system.kernel = payload;
  hostil.packages = [{ name: payload, version: "1-1" }];
  const { page, errors, close } = await openApp(browser, app, { handlers: { read_nodusctl: JSON.stringify(hostil) } });
  await page.getByTestId("system").waitFor();
  await page.getByTestId("packages-toggle").click();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("kernel").innerText(), /<img src=x/);
  assert.match(await page.getByTestId("packages-list").innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});
