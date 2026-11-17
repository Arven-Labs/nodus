// As abas e a tela de repositórios num navegador, com o backend de mentira (ADR 0026). Prova o que cada aba pede ao backend
// (só `read_nodusctl` com o nome do comando, nada que escreva) e como a tela trata dados e erros. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const reposText = readFileSync(new URL("../tests/fixtures/repos.json", import.meta.url), "utf8");
const reposFixture = () => JSON.parse(reposText);

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

/** Responde cada comando com o seu JSON: `inventory` e `repos` (ou o que o teste trocar). */
const handlers = (repos = reposText) => ({
  read_nodusctl: ({ command }) => (command === "inventory" ? inventoryText : command === "repos" ? repos : { __error: { kind: "not_allowed", message: command } }),
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

test("abrir o app pede só o inventário (a aba Repositórios não carrega antes de ser aberta)", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  assert.deepEqual(errors, []);
  await close();
});

test("a aba Repositórios pede `repos` e lista os repositórios", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("repos-list").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "repos"]);
  const itens = page.getByTestId("repo");
  assert.equal(await itens.count(), 4);
  assert.match(await itens.nth(0).innerText(), /alpha/);
  assert.match(await itens.nth(0).innerText(), /main/);
  assert.match(await itens.nth(0).innerText(), /2 à frente, 0 atrás/);
  assert.deepEqual(errors, []);
  await close();
});

test("sem upstream mostra 'sem upstream' (e não 0)", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("repos-list").waitFor();
  const beta = await page.getByTestId("repo").nth(1).innerText();
  assert.match(beta, /sem upstream/);
  assert.doesNotMatch(beta, /à frente/);
  await close();
});

test("upstream removido é dito", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("repos-list").waitFor();
  assert.match(await page.getByTestId("repo").nth(2).innerText(), /upstream removido/);
  await close();
});

test("o erro de um repositório aparece nele e os outros continuam", async () => {
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("repos-list").waitFor();
  assert.match(await page.getByTestId("repo").nth(3).innerText(), /git terminou com status 128/);
  assert.match(await page.getByTestId("repo").nth(0).innerText(), /alpha/);
  assert.deepEqual(errors, []);
  await close();
});

test("workspace sem repositórios: mensagem própria", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers("[]") });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("repos-vazio").waitFor();
  assert.match(await page.getByTestId("repos-vazio").innerText(), /nenhum repositório encontrado/i);
  await close();
});

test("nomes, branch e URL hostis viram texto: nada é executado nem vira elemento", async () => {
  const hostil = reposFixture();
  const payload = '<img src=x onerror="window.__pwned=1">';
  hostil[0].name = payload;
  hostil[0].branch = payload;
  hostil[0].origin_url = payload;
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(hostil)) });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("repos-list").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("repo").nth(0).innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("JSON inesperado: erro visível com o campo, e o inventário continua acessível", async () => {
  const quebrado = reposFixture();
  delete quebrado[0].name;
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(quebrado)) });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /repos\[0\]\.name/);
  await page.getByTestId("tab-inventory").click();
  await page.getByTestId("system").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("backend com versão incompatível: mesma tela de bloqueio, sem lista", async () => {
  const erro = { kind: "version_mismatch", message: "nodusctl 0.2.0 não é compatível com esta interface (0.1.x)" };
  const h = {
    read_nodusctl: ({ command }) => (command === "inventory" ? inventoryText : { __error: erro }),
  };
  const { page, close } = await openApp(browser, app, { handlers: h });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("versao-incompativel").waitFor();
  assert.equal(await page.getByTestId("repos-list").count(), 0);
  await close();
});

test("nenhuma aba pede comando que escreve: só read_nodusctl", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("tab-repos").click();
  await page.getByTestId("repos-list").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  await close();
});
