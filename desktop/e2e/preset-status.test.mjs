// A aba Status num navegador, com o backend de mentira (ADR 0026). Prova que a aba pede só `preset_status`, só depois de aberta
// (e nunca manda caminho nem rótulo: o arquivo vem de NODUS_PRESET, no Rust), que mostra por máquina o que falta e o que sobra,
// o Flatpak "não comparável" sem chamá-lo de diferença, o snapshot ilegível, o caso sem snapshots, o erro do backend, a nota
// sobre o `pacman -Qe` e que dado hostil vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const statusText = readFileSync(new URL("../tests/fixtures/preset_status.json", import.meta.url), "utf8");
const statusFixture = () => JSON.parse(statusText);

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

const handlers = (status = statusText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "preset_status" ? status : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-status").click();
}

test("a aba Status só pede `preset_status` depois de aberta, sem mandar caminho nem rótulo", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  await page.getByTestId("tab-status").click();
  await page.getByTestId("status-maquina").first().waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "preset_status"]);
  for (const c of calls) assert.deepEqual(Object.keys(c.args), ["command"], "só o nome do comando: o caminho vem do Rust");
  assert.deepEqual(errors, []);
  await close();
});

test("uma seção por máquina, com o nome e o sistema", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("status-maquina").first().waitFor();
  assert.equal(await page.getByTestId("status-maquina").count(), 2);
  const primeira = await page.getByTestId("status-maquina").nth(0).innerText();
  assert.match(primeira, /archlinux/);
  assert.match(primeira, /Arch Linux/);
  assert.match(await page.getByTestId("status-maquina").nth(1).innerText(), /pc-fake/);
  await close();
});

test("mostra o que falta no preset e o que a máquina tem fora dele, em listas separadas", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const maquina = page.getByTestId("status-maquina").nth(0);
  await maquina.waitFor();
  assert.equal(await maquina.getByTestId("status-faltam-item").count(), 1);
  assert.match(await maquina.getByTestId("status-faltam").innerText(), /htop/);
  assert.equal(await maquina.getByTestId("status-sobram-item").count(), 2);
  const sobram = await maquina.getByTestId("status-sobram").innerText();
  assert.match(sobram, /vim/);
  assert.match(sobram, /tmux/);
  assert.doesNotMatch(sobram, /htop/, "o que falta não se mistura com o que sobra");
  assert.equal(await maquina.getByTestId("status-em-dia").count(), 0, "com diferença não é 'em dia'");
  await close();
});

test("lista não comparável não é diferença: diz que a máquina não tem a capacidade e a máquina fica 'em dia nas listas comparadas'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const maquina = page.getByTestId("status-maquina").nth(1);
  await maquina.waitFor();
  const aviso = await maquina.getByTestId("status-nao-comparavel").innerText();
  assert.match(aviso, /não comparável/i);
  assert.match(aviso, /pc-fake não tem Flatpak/);
  assert.equal(await maquina.getByTestId("status-faltam-item").count(), 0);
  assert.match(await maquina.getByTestId("status-em-dia").innerText(), /em dia nas listas comparadas/i);
  await close();
});

test("tudo comparado e sem diferenças: 'em dia com o preset'", async () => {
  const data = statusFixture();
  data.machines = [data.machines[0]];
  data.machines[0].pacman = { status: "compared", missing: [], extra: [], in_sync: 3 };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  const maquina = page.getByTestId("status-maquina").nth(0);
  await maquina.waitFor();
  assert.match(await maquina.getByTestId("status-em-dia").innerText(), /em dia com o preset/i);
  await close();
});

test("snapshot ilegível vira aviso com o rótulo e o motivo, sem esconder as máquinas lidas", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("status-ilegivel").waitFor();
  const texto = await page.getByTestId("status-ilegivel").innerText();
  assert.match(texto, /quebrado/);
  assert.match(texto, /JSON inválido/);
  assert.equal(await page.getByTestId("status-maquina").count(), 2);
  await close();
});

test("sem nenhum snapshot: diz como gerar, e não aparece 'em dia'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify({ machines: [], unreadable: [] })) });
  await abrirAba(page);
  await page.getByTestId("status-vazio").waitFor();
  const texto = await page.getByTestId("status-vazio").innerText();
  assert.match(texto, /nenhum snapshot/i);
  assert.match(texto, /nodusctl snapshot/);
  assert.equal(await page.getByTestId("status-em-dia").count(), 0);
  await close();
});

test("com só snapshots ilegíveis, não diz 'nenhum snapshot'", async () => {
  const data = { machines: [], unreadable: [{ label: "x", reason: "ilegível" }] };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("status-ilegivel").waitFor();
  assert.equal(await page.getByTestId("status-vazio").count(), 0);
  await close();
});

test("a nota diz que é só leitura e que 'não instalado de propósito' inclui o que veio como dependência", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("status-nota").waitFor();
  const texto = await page.getByTestId("status-nota").innerText();
  assert.match(texto, /só leitura/i);
  assert.match(texto, /dependência/i);
  assert.match(texto, /pacman -Qe/);
  await close();
});

test("NODUS_PRESET ausente: a mensagem do backend aparece como erro, e as outras abas continuam", async () => {
  const h = {
    read_nodusctl: ({ command }) =>
      command === "inventory"
        ? inventoryText
        : { __error: { kind: "preset_not_set", message: "defina NODUS_PRESET com o caminho absoluto do arquivo de preset" } },
  };
  const { page, errors, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /NODUS_PRESET/);
  await page.getByTestId("tab-inventory").click();
  await page.getByTestId("system").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("nomes hostis (máquina, pacote, rótulo, motivo) viram texto: nada é executado nem vira elemento", async () => {
  const payload = '<img src=x onerror="window.__pwned=1">';
  const data = statusFixture();
  data.machines[0].machine = payload;
  data.machines[0].pacman.missing = [payload];
  data.machines[0].pacman.extra = [payload];
  data.unreadable = [{ label: payload, reason: payload }];
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("status-maquina").first().waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("status-maquina").first().innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("a aba só pede read_nodusctl: nada que escreva", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("status-maquina").first().waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  await close();
});
