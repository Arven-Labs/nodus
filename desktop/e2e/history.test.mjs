// A aba Histórico num navegador, com o backend de mentira (ADR 0026). Prova que a aba pede só `history` (nunca `history undo`),
// só depois de aberta, que lista do mais novo para o mais antigo mantendo o número que o `nodusctl history` usa, como cada ação,
// resultado e reversão aparecem, a nota de que o histórico registra só o pedido, o caso vazio, o erro e que dado hostil vira
// texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const historyText = readFileSync(new URL("../tests/fixtures/history.json", import.meta.url), "utf8");
const historyFixture = () => JSON.parse(historyText);

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

const handlers = (history = historyText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "history" ? history : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-history").click();
}

test("a aba Histórico só pede `history` depois de aberta, e nada que escreva", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  await page.getByTestId("tab-history").click();
  await page.getByTestId("history-list").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "history"]);
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  assert.deepEqual(errors, []);
  await close();
});

test("lista do mais novo para o mais antigo, com o número da entrada no `nodusctl history`", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("history-list").waitFor();
  const itens = page.getByTestId("history-entry");
  assert.equal(await itens.count(), 5);
  assert.match(await itens.nth(0).innerText(), /#5\b/);
  assert.match(await itens.nth(4).innerText(), /#1\b/);
  await close();
});

test("cada ação tem o seu texto, com o que foi pedido e a data em UTC", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("history-list").waitFor();
  const itens = page.getByTestId("history-entry");
  const link = await itens.nth(0).innerText();
  assert.match(link, /dotfile ligado/i);
  assert.match(link, /\.exemplo/);
  assert.match(link, /\/home\/exemplo\/dotfiles/);
  assert.match(await itens.nth(1).innerText(), /dotfile adotado/i);
  const remocao = await itens.nth(2).innerText();
  assert.match(remocao, /remoção/i);
  assert.match(remocao, /desfez #1/);
  const flatpak = await itens.nth(3).innerText();
  assert.match(flatpak, /instalação/i);
  assert.match(flatpak, /flatpak/i);
  assert.match(flatpak, /org\.exemplo\.App/);
  const pacman = await itens.nth(4).innerText();
  assert.match(pacman, /sl, tree/);
  assert.match(pacman, /2023-11-14 22:13:20 UTC/);
  await close();
});

test("resultado: ok, falhou com código e falhou sem código", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("history-list").waitFor();
  const itens = page.getByTestId("history-entry");
  assert.match(await itens.nth(4).innerText(), /\bok\b/);
  assert.match(await itens.nth(3).innerText(), /falhou \(código 1\)/);
  const semCodigo = await itens.nth(1).innerText();
  assert.match(semCodigo, /falhou/);
  assert.doesNotMatch(semCodigo, /código/);
  await close();
});

test("mostra o comando exato que foi executado, como texto", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("history-list").waitFor();
  assert.match(await page.getByTestId("history-entry").nth(4).innerText(), /sudo pacman -S --needed -- sl tree/);
  await close();
});

test("a nota diz que o histórico registra só o pedido, não as dependências", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("history-nota").waitFor();
  assert.match(await page.getByTestId("history-nota").innerText(), /só os pacotes pedidos/i);
  await close();
});

test("histórico vazio: mensagem própria, sem lista", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers("[]") });
  await abrirAba(page);
  await page.getByTestId("history-vazio").waitFor();
  assert.match(await page.getByTestId("history-vazio").innerText(), /nenhuma execução registrada/i);
  assert.equal(await page.getByTestId("history-list").count(), 0);
  await close();
});

test("erro do backend aparece como erro, e as outras abas continuam", async () => {
  const h = {
    read_nodusctl: ({ command }) =>
      command === "inventory" ? inventoryText : { __error: { kind: "failed", message: "linha 3 do histórico: versão não suportada" } },
  };
  const { page, errors, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /versão não suportada/);
  await page.getByTestId("tab-inventory").click();
  await page.getByTestId("system").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("JSON inesperado: erro visível com o campo", async () => {
  const quebrado = historyFixture();
  quebrado[0].action = "purge";
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(quebrado)) });
  await abrirAba(page);
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /action/);
  assert.deepEqual(errors, []);
  await close();
});

test("nomes, comando e pasta hostis viram texto: nada é executado nem vira elemento", async () => {
  const hostil = historyFixture();
  const payload = '<img src=x onerror="window.__pwned=1">';
  hostil[0].requested = [payload];
  hostil[0].command = ["sudo", payload];
  hostil[3].dir = payload;
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(hostil)) });
  await abrirAba(page);
  await page.getByTestId("history-list").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("history-entry").nth(4).innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("versão incompatível: mesma tela de bloqueio, sem lista", async () => {
  const erro = { kind: "version_mismatch", message: "nodusctl 0.2.0 não é compatível com esta interface (0.1.x)" };
  const h = { read_nodusctl: ({ command }) => (command === "inventory" ? inventoryText : { __error: erro }) };
  const { page, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("versao-incompativel").waitFor();
  assert.equal(await page.getByTestId("history-list").count(), 0);
  await close();
});
