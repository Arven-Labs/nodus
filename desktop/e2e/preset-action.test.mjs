// A ação "Instalar no terminal" da aba Preset num navegador, com o backend de mentira (ADR 0027). Prova que o botão só aparece
// quando há algo a instalar, que o clique manda só o NOME da ação (o terminal, o caminho e o `sudo` são do Rust e da CLI), que
// não abre duas janelas por clique duplo, que "Atualizar" relê o plano, que sem terminal a tela mostra o comando como texto
// (com Copiar) e que dado hostil vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const planText = readFileSync(new URL("../tests/fixtures/preset_plan.json", import.meta.url), "utf8");
const planFixture = () => JSON.parse(planText);

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

const COMANDO = "/usr/bin/nodusctl preset apply /tmp/preset.toml";

/** `plans`: o que cada chamada sucessiva de `preset_plan` devolve (a última se repete). `action`: o que `run_action` devolve. */
const handlers = ({ plans = [planText], action = { launched: true, command: COMANDO } } = {}) => {
  let n = 0;
  return {
    read_nodusctl: ({ command }) => {
      if (command === "inventory") return inventoryText;
      if (command === "preset_plan") return plans[Math.min(n++, plans.length - 1)];
      return { __error: { kind: "not_allowed", message: command } };
    },
    run_action: typeof action === "function" ? action : () => action,
  };
};
const leituras = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);
const acoes = (calls) => calls.filter((c) => c.cmd === "run_action");

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-preset").click();
  await page.getByTestId("preset-pacman").waitFor();
}

test("há o que instalar: o botão aparece e ainda nada foi lançado", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").waitFor();
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("o botão só aparece quando há algo que o `apply` instalaria", async () => {
  const casos = [
    ["nada faltando", { ...planFixture(), pacman_missing: [], flatpak_missing: [], pacman_unavailable: [] }, false],
    ["só indisponíveis", { ...planFixture(), pacman_missing: [], flatpak_missing: [] }, false],
    ["só pacman faltando", { ...planFixture(), flatpak_missing: [] }, true],
    ["só Flatpak faltando, Flatpak instalado", { ...planFixture(), pacman_missing: [] }, true],
    ["só Flatpak faltando, Flatpak ausente", { ...planFixture(), pacman_missing: [], flatpak_installed: false }, false],
  ];
  for (const [nome, plano, aparece] of casos) {
    const { page, close } = await openApp(browser, app, { handlers: handlers({ plans: [JSON.stringify(plano)] }) });
    await abrirAba(page);
    assert.equal(await page.getByTestId("preset-instalar").count(), aparece ? 1 : 0, nome);
    await close();
  }
});

test("o clique manda só o nome da ação: nenhum caminho, comando ou argumento", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-acao-ok").waitFor();
  const feitas = acoes(calls);
  assert.equal(feitas.length, 1);
  assert.deepEqual(feitas[0].args, { command: "preset_apply" });
  assert.deepEqual(errors, []);
  await close();
});

test("terminal aberto: diz para confirmar lá e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  const texto = await page.getByTestId("preset-acao-ok").innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /confirm/i);
  assert.match(texto, /atualizar/i);
  assert.equal(await page.getByTestId("preset-instalar").isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").dblclick();
  await page.getByTestId("preset-acao-ok").waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê o plano, e o que já foi instalado some da tela", async () => {
  const depois = { ...planFixture(), pacman_missing: [], flatpak_missing: [], pacman_unavailable: [] };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ plans: [planText, JSON.stringify(depois)] }) });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-atualizar").click();
  await page.getByTestId("preset-nada").waitFor();
  assert.deepEqual(leituras(calls), ["inventory", "preset_plan", "preset_plan"]);
  assert.equal(await page.getByTestId("preset-instalar").count(), 0);
  assert.equal(await page.getByTestId("preset-acao-ok").count(), 0, "a mensagem da ação anterior não sobra");
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl preset apply '/tmp/minha pasta/p.toml'";
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-comando").waitFor();
  assert.equal(await page.getByTestId("preset-comando").innerText(), comando);
  assert.match(await page.getByTestId("preset-sem-terminal").innerText(), /nenhum terminal/i);
  assert.equal(await page.getByTestId("preset-acao-ok").count(), 0);
  assert.equal(await page.getByTestId("preset-instalar").isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("Copiar põe o comando na área de transferência", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: COMANDO } }) });
  await abrirAba(page);
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (t) => void (window.__copiado = t) } });
  });
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-copiar").click();
  await page.waitForFunction(() => window.__copiado !== undefined);
  assert.equal(await page.evaluate(() => window.__copiado), COMANDO);
  await close();
});

test("se a cópia falhar, avisa para selecionar o texto, sem erro na página", async () => {
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: COMANDO } }) });
  await abrirAba(page);
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async () => Promise.reject(new Error("negado")) } });
  });
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-copiar").click();
  await page.getByTestId("preset-copiar-erro").waitFor();
  assert.match(await page.getByTestId("preset-copiar-erro").innerText(), /selecione/i);
  assert.deepEqual(errors, []);
  await close();
});

test("erro do backend aparece com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "invalid_terminal", message: "NODUS_TERMINAL precisa ser o caminho absoluto de um terminal conhecido" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-acao-erro").waitFor();
  assert.match(await page.getByTestId("preset-acao-erro").innerText(), /NODUS_TERMINAL/);
  assert.equal(await page.getByTestId("preset-instalar").isDisabled(), false);
  assert.equal(await page.getByTestId("preset-pacman").count(), 1, "o plano continua na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("resposta inesperada do backend vira erro visível, não tela quebrada", async () => {
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: "sim" } }) });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-acao-erro").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-comando").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await page.getByTestId("preset-comando").innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a nota da aba diz que a janela não instala e que a confirmação é no terminal", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const texto = await page.getByTestId("preset-nota").innerText();
  assert.match(texto, /não instala nada/i);
  assert.match(texto, /abre um terminal/i);
  assert.match(texto, /preset apply/);
  assert.match(texto, /confirmação/i);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-instalar").click();
  await page.getByTestId("preset-acao-ok").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
