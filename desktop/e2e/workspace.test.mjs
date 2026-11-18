// A aba Workspace num navegador, com o backend de mentira (ADR 0026). Prova que a aba pede só `workspace`, só depois de aberta, que
// mostra a pasta escolhida com a origem e a contagem, os candidatos, o caso "nenhum workspace", o erro do backend e que dado
// hostil vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const workspaceText = readFileSync(new URL("../tests/fixtures/workspace.json", import.meta.url), "utf8");
const workspaceFixture = () => JSON.parse(workspaceText);

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

const handlers = (workspace = workspaceText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "workspace" ? workspace : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-workspace").click();
}

test("a aba Workspace só pede `workspace` depois de aberta", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  await page.getByTestId("tab-workspace").click();
  await page.getByTestId("workspace-escolhido").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "workspace"]);
  assert.deepEqual(errors, []);
  await close();
});

test("mostra a pasta escolhida, a origem e a contagem de repositórios", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-escolhido").waitFor();
  const texto = await page.getByTestId("workspace-escolhido").innerText();
  assert.match(texto, /\/home\/exemplo\/ws/);
  assert.match(texto, /3 repositórios/);
  assert.match(texto, /detectado/i);
  await close();
});

test("lista os candidatos com a contagem de cada um", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-escolhido").waitFor();
  assert.equal(await page.getByTestId("workspace-candidato").count(), 2);
  const lista = await page.getByTestId("workspace-candidatos").innerText();
  assert.match(lista, /\/home\/exemplo\/Projects/);
  assert.match(lista, /0 repositórios/);
  await close();
});

test("as origens têm nome em português", async () => {
  const nomes = { informed: /informado/i, environment: /NODUS_WORKSPACE/, stored: /gravado/i, detected: /detectado/i };
  for (const [source, esperado] of Object.entries(nomes)) {
    const data = workspaceFixture();
    data.chosen.source = source;
    const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
    await abrirAba(page);
    await page.getByTestId("workspace-escolhido").waitFor();
    assert.match(await page.getByTestId("workspace-escolhido").innerText(), esperado, source);
    await close();
  }
});

test("nada escolhido: mensagem própria, e os candidatos (se houver) continuam à mostra", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify({ chosen: null, candidates: [] })) });
  await abrirAba(page);
  await page.getByTestId("workspace-nenhum").waitFor();
  assert.match(await page.getByTestId("workspace-nenhum").innerText(), /nenhum workspace/i);
  assert.equal(await page.getByTestId("workspace-escolhido").count(), 0);
  assert.equal(await page.getByTestId("workspace-candidato").count(), 0);
  await close();
});

test("um repositório no singular", async () => {
  const data = workspaceFixture();
  data.chosen.repo_count = 1;
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("workspace-escolhido").waitFor();
  assert.match(await page.getByTestId("workspace-escolhido").innerText(), /1 repositório(?!s)/);
  await close();
});

test("erro do backend aparece como erro, e as outras abas continuam", async () => {
  const h = {
    read_nodusctl: ({ command }) =>
      command === "inventory" ? inventoryText : { __error: { kind: "failed", message: "NODUS_WORKSPACE precisa ser um caminho absoluto" } },
  };
  const { page, errors, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /caminho absoluto/);
  await page.getByTestId("tab-inventory").click();
  await page.getByTestId("system").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("caminhos hostis viram texto: nada é executado nem vira elemento", async () => {
  const hostil = workspaceFixture();
  const payload = '<img src=x onerror="window.__pwned=1">';
  hostil.chosen.path = payload;
  hostil.candidates[0].path = payload;
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(hostil)) });
  await abrirAba(page);
  await page.getByTestId("workspace-escolhido").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("workspace-escolhido").innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("a aba só pede read_nodusctl: nada que escreva", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-escolhido").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  await close();
});
