// A aba Atualizações num navegador, com o backend de mentira (ADR 0026). Prova que abrir a aba NÃO roda nada (usa a rede), que o
// botão "Verificar" pede só `updates`, que a tela mostra "Verificando…" enquanto espera (até 120 s no Rust), o aviso de lista
// possivelmente desatualizada, o Flatpak ausente, o erro de tempo esgotado e que dado hostil vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const updatesText = readFileSync(new URL("../tests/fixtures/updates.json", import.meta.url), "utf8");
const updatesFixture = () => JSON.parse(updatesText);

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

/** Responde `inventory` e `updates` (ou o que o teste trocar). */
const handlers = (updates = updatesText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "updates" ? updates : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-updates").click();
}

test("abrir a aba Atualizações não roda nada (usa a rede): só o botão", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").waitFor();
  await page.waitForTimeout(200);
  assert.deepEqual(comandos(calls), ["inventory"]);
  assert.equal(await page.getByTestId("updates-resultado").count(), 0);
  assert.deepEqual(errors, []);
  await close();
});

test("o botão pede `updates` e lista pacman e Flatpak", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "updates"]);
  const pacman = await page.getByTestId("updates-pacman").innerText();
  assert.match(pacman, /alpha-lib/);
  assert.match(pacman, /1\.0-1/);
  assert.match(pacman, /1\.1-1/);
  assert.equal(await page.getByTestId("update-pacman").count(), 2);
  const flatpak = await page.getByTestId("updates-flatpak").innerText();
  assert.match(flatpak, /org\.exemplo\.App/);
  assert.match(flatpak, /4\.2/);
  assert.equal(await page.getByTestId("update-flatpak").count(), 2);
  assert.equal(await page.getByTestId("updates-aviso-desatualizado").count(), 0);
  assert.deepEqual(errors, []);
  await close();
});

test("enquanto espera mostra 'Verificando…' e o botão não deixa disparar duas vezes", async () => {
  let liberar;
  const espera = new Promise((resolve) => (liberar = resolve));
  const h = {
    read_nodusctl: async ({ command }) => {
      if (command === "inventory") return inventoryText;
      await espera;
      return updatesText;
    },
  };
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-carregando").waitFor();
  assert.equal(await page.getByTestId("updates-verificar").isDisabled(), true);
  await page.getByTestId("updates-verificar").click({ force: true }).catch(() => {});
  liberar();
  await page.getByTestId("updates-resultado").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "updates"], "um só pedido de updates");
  assert.deepEqual(errors, []);
  await close();
});

test("'Verificar de novo' refaz o pedido", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "updates", "updates"]);
  await close();
});

test("`pacman-qu` mostra o aviso de lista possivelmente desatualizada", async () => {
  const data = updatesFixture();
  data.pacman_source = "pacman-qu";
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-aviso-desatualizado").waitFor();
  assert.match(await page.getByTestId("updates-aviso-desatualizado").innerText(), /pode estar desatualizado/i);
  await close();
});

test("Flatpak não instalado é dito, e não vira lista vazia", async () => {
  const data = updatesFixture();
  data.flatpak = null;
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  assert.match(await page.getByTestId("updates-flatpak").innerText(), /flatpak não instalado/i);
  assert.equal(await page.getByTestId("update-flatpak").count(), 0);
  await close();
});

test("tudo em dia: mensagem própria nas duas listas", async () => {
  const data = updatesFixture();
  data.pacman = [];
  data.flatpak = [];
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  assert.match(await page.getByTestId("updates-pacman").innerText(), /nenhuma atualização/i);
  assert.match(await page.getByTestId("updates-flatpak").innerText(), /nenhuma atualização/i);
  await close();
});

test("tempo esgotado aparece como erro, e dá para tentar de novo", async () => {
  let tentativas = 0;
  const h = {
    read_nodusctl: ({ command }) => {
      if (command === "inventory") return inventoryText;
      tentativas += 1;
      return tentativas === 1
        ? { __error: { kind: "timeout", message: "o nodusctl demorou mais de 120 s e foi interrompido" } }
        : updatesText;
    },
  };
  const { page, errors, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /120 s/);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  assert.equal(await page.getByTestId("erro").count(), 0);
  assert.deepEqual(errors, []);
  await close();
});

test("nomes e versões hostis viram texto: nada é executado nem vira elemento", async () => {
  const hostil = updatesFixture();
  const payload = '<img src=x onerror="window.__pwned=1">';
  hostil.pacman[0].name = payload;
  hostil.pacman[0].new_version = payload;
  hostil.flatpak[0].application = payload;
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(hostil)) });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("update-pacman").nth(0).innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("JSON inesperado: erro visível com o campo, e as outras abas continuam", async () => {
  const quebrado = updatesFixture();
  delete quebrado.pacman_source;
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(quebrado)) });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /pacman_source/);
  await page.getByTestId("tab-inventory").click();
  await page.getByTestId("system").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("versão incompatível: mesma tela de bloqueio, sem lista", async () => {
  const erro = { kind: "version_mismatch", message: "nodusctl 0.2.0 não é compatível com esta interface (0.1.x)" };
  const h = { read_nodusctl: ({ command }) => (command === "inventory" ? inventoryText : { __error: erro }) };
  const { page, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("versao-incompativel").waitFor();
  assert.equal(await page.getByTestId("updates-resultado").count(), 0);
  await close();
});

test("a aba só pede read_nodusctl: nada que escreva", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("updates-verificar").click();
  await page.getByTestId("updates-resultado").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  await close();
});
