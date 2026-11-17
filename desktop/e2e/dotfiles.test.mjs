// A aba de dotfiles num navegador, com o backend de mentira (ADR 0026). Prova o que ela pede ao backend (só `read_nodusctl`
// com `dotfiles`, nunca `adopt` nem `link`) e como a tela mostra dados e erros. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const dotfilesText = readFileSync(new URL("../tests/fixtures/dotfiles.json", import.meta.url), "utf8");
const dotfilesFixture = () => JSON.parse(dotfilesText);

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

const handlers = (dotfiles = dotfilesText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "dotfiles" ? dotfiles : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);
const abrir = async (h) => {
  const aberto = await openApp(browser, app, { handlers: h });
  await aberto.page.getByTestId("tab-dotfiles").click();
  return aberto;
};

test("abrir o app não pede dotfiles; a aba pede só `dotfiles`", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  await page.getByTestId("tab-dotfiles").click();
  await page.getByTestId("dotfiles-list").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "dotfiles"]);
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  assert.deepEqual(errors, []);
  await close();
});

test("lista cada estado: arquivo, symlink (com e sem aviso), ausente, pasta e outro", async () => {
  const { page, close } = await abrir(handlers());
  await page.getByTestId("dotfiles-list").waitFor();
  const itens = page.getByTestId("dotfile");
  assert.equal(await itens.count(), 6);
  assert.match(await itens.nth(0).innerText(), /\.bashrc.*arquivo \(1234 bytes\)/s);
  const comAviso = await itens.nth(1).innerText();
  assert.match(comAviso, /\/dotfiles-teste\/\.gitconfig/);
  assert.match(comAviso, /fora do HOME/);
  const semAviso = await itens.nth(2).innerText();
  assert.match(semAviso, /\.dotfiles-teste\/\.profile/);
  assert.doesNotMatch(semAviso, /fora do HOME/);
  assert.match(await itens.nth(3).innerText(), /ausente/);
  assert.match(await itens.nth(4).innerText(), /pasta/);
  assert.match(await itens.nth(5).innerText(), /outro tipo/);
  await close();
});

test("resumo: presentes de total", async () => {
  const { page, close } = await abrir(handlers());
  await page.getByTestId("dotfiles-list").waitFor();
  assert.match(await page.getByTestId("dotfiles-resumo").innerText(), /Presentes: 5 de 6/);
  await close();
});

test("lista vazia: mensagem própria", async () => {
  const { page, close } = await abrir(handlers("[]"));
  await page.getByTestId("dotfiles-vazio").waitFor();
  assert.match(await page.getByTestId("dotfiles-vazio").innerText(), /Nenhum dotfile na lista/);
  await close();
});

test("caminho e alvo hostis viram texto: nada é executado nem vira elemento", async () => {
  const hostil = dotfilesFixture();
  const payload = '<img src=x onerror="window.__pwned=1">';
  hostil[0].path = payload;
  hostil[1].target = payload;
  const { page, errors, close } = await abrir(handlers(JSON.stringify(hostil)));
  await page.getByTestId("dotfiles-list").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("dotfile").nth(0).innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("kind desconhecido: erro visível com o campo, e o inventário continua acessível", async () => {
  const quebrado = dotfilesFixture();
  quebrado[2].kind = "weird";
  const { page, errors, close } = await abrir(handlers(JSON.stringify(quebrado)));
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /dotfiles\[2\]\.kind/);
  assert.equal(await page.getByTestId("dotfiles-list").count(), 0);
  await page.getByTestId("tab-inventory").click();
  await page.getByTestId("system").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("versão incompatível: mesma tela de bloqueio, sem lista", async () => {
  const erro = { kind: "version_mismatch", message: "nodusctl 0.2.0 não é compatível com esta interface (0.1.x)" };
  const h = { read_nodusctl: ({ command }) => (command === "inventory" ? inventoryText : { __error: erro }) };
  const { page, close } = await abrir(h);
  await page.getByTestId("versao-incompativel").waitFor();
  assert.equal(await page.getByTestId("dotfiles-list").count(), 0);
  await close();
});
