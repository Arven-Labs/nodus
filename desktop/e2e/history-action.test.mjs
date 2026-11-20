// A ação "Desfazer no terminal" da aba Histórico num navegador, com o backend de mentira (ADR 0027). Prova em quais entradas o
// botão aparece (só onde a CLI desfaz: instalação pacman e dotfile adotado ou ligado, que ainda não foi desfeito), que o clique manda
// o NOME da ação e o número `n` da entrada clicada (o mesmo do `nodusctl history`, posição + 1, com a lista invertida na tela), que
// não abre duas janelas por clique duplo, que "Atualizar" relê o histórico, o fallback com o comando, o erro e que dado hostil vira
// texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const historyText = readFileSync(new URL("../tests/fixtures/history.json", import.meta.url), "utf8");

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

const entrada = (over) => ({
  version: 3,
  at_unix: 1700000000,
  command: ["sudo", "pacman", "-S", "--needed", "--", "sl"],
  requested: ["sl"],
  installed: ["sl"],
  still_missing: [],
  result: { status: "ok" },
  action: "install",
  manager: "pacman",
  ...over,
});
const remocao = (undoes, result = { status: "ok" }, manager = "pacman") =>
  entrada({ action: "remove", undoes, result, manager, command: ["sudo", "pacman", "-Rs", "--", "sl"], installed: [] });
const dotfile = (action, over = {}) =>
  entrada({ version: 4, action, manager: "dotfiles", dir: "/home/exemplo/dotfiles", command: ["nodusctl", "dotfiles", action, ".exemplo"], ...over });

/**
 * #1 instalação já desfeita (pela #4) | #2 instalação que falhou | #3 Flatpak | #4 reversão | #5 instalação (a reversão #6 falhou) |
 * #6 reversão que falhou | #7 dotfile adotado | #8 dotfile ligado, já desfeito (pela #9) | #9 reversão de dotfile.
 * Desfazíveis: #2, #3 (Flatpak), #5, #7.
 */
const cenario = () => [
  entrada({}),
  entrada({ result: { status: "failed", code: 1 }, installed: [], still_missing: ["sl"] }),
  entrada({ manager: "flatpak", command: ["flatpak", "install", "--user", "--", "flathub", "org.exemplo.App"], requested: ["org.exemplo.App"] }),
  remocao(1),
  entrada({}),
  remocao(5, { status: "failed", code: 1 }),
  dotfile("adopt"),
  dotfile("link"),
  remocao(8, { status: "ok" }, "dotfiles"),
];

const COMANDO = "/usr/bin/nodusctl history undo 5";

/** `histories`: o que cada chamada sucessiva de `history` devolve (a última se repete). */
const handlers = ({ histories = [JSON.stringify(cenario())], action = { launched: true, command: COMANDO } } = {}) => {
  let n = 0;
  return {
    read_nodusctl: ({ command }) => {
      if (command === "inventory") return inventoryText;
      if (command === "history") return histories[Math.min(n++, histories.length - 1)];
      return { __error: { kind: "not_allowed", message: command } };
    },
    run_action: typeof action === "function" ? action : () => action,
  };
};
const leituras = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);
const acoes = (calls) => calls.filter((c) => c.cmd === "run_action");

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-history").click();
  await page.getByTestId("history-list").waitFor();
}

/** A entrada de número `n` (a lista da tela é do mais novo para o mais antigo). */
const daEntrada = (page, n) => page.getByTestId("history-entry").filter({ has: page.getByText(`#${n}`, { exact: true }) });

test("o botão aparece só onde a CLI desfaz: instalação pacman, Flatpak e dotfile, ainda não desfeitos", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const com = [];
  for (let n = 1; n <= 9; n++) if ((await daEntrada(page, n).getByTestId("history-desfazer").count()) === 1) com.push(n);
  assert.deepEqual(com, [2, 3, 5, 7]);
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("com o histórico de exemplo, podem ser desfeitas a instalação Flatpak e as entradas de dotfile", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers({ histories: [historyText] }) });
  await abrirAba(page);
  const com = [];
  for (let n = 1; n <= 5; n++) if ((await daEntrada(page, n).getByTestId("history-desfazer").count()) === 1) com.push(n);
  // #1 já foi desfeita pela #3; #2 é instalação Flatpak (falhou, mas a CLI é quem recusa); #3 é reversão; #4 e #5 são dotfile adotado e ligado.
  assert.deepEqual(com, [2, 4, 5]);
  await close();
});

test("o clique manda o nome da ação e o número da entrada clicada, não a posição na tela", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await daEntrada(page, 5).getByTestId("history-desfazer").click();
  await daEntrada(page, 5).getByTestId("history-acao-ok").waitFor();
  const feitas = acoes(calls);
  assert.equal(feitas.length, 1);
  assert.deepEqual(feitas[0].args, { command: "history_undo", n: 5 });
  assert.deepEqual(errors, []);
  await close();
});

test("cada entrada manda o seu próprio número", async () => {
  for (const n of [2, 3, 7]) {
    const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
    await abrirAba(page);
    await daEntrada(page, n).getByTestId("history-desfazer").click();
    await daEntrada(page, n).getByTestId("history-acao-ok").waitFor();
    assert.deepEqual(acoes(calls)[0].args, { command: "history_undo", n });
    await close();
  }
});

test("terminal aberto: diz para confirmar lá, e o botão daquela entrada fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const item = daEntrada(page, 5);
  await item.getByTestId("history-desfazer").click();
  const texto = await item.getByTestId("history-acao-ok").innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /confirm/i);
  assert.match(texto, /atualizar/i);
  assert.equal(await item.getByTestId("history-desfazer").isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await daEntrada(page, 5).getByTestId("history-desfazer").dblclick();
  await daEntrada(page, 5).getByTestId("history-acao-ok").waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê o histórico, e a entrada já desfeita perde o botão", async () => {
  const depois = [...cenario(), remocao(5)];
  const { page, calls, close } = await openApp(browser, app, {
    handlers: handlers({ histories: [JSON.stringify(cenario()), JSON.stringify(depois)] }),
  });
  await abrirAba(page);
  await daEntrada(page, 5).getByTestId("history-desfazer").click();
  await daEntrada(page, 5).getByTestId("history-atualizar").click();
  await page.getByTestId("history-list").waitFor();
  await daEntrada(page, 10).waitFor();
  assert.deepEqual(leituras(calls), ["inventory", "history", "history"]);
  assert.equal(await daEntrada(page, 5).getByTestId("history-desfazer").count(), 0, "a #5 agora foi desfeita pela #10");
  assert.equal(await daEntrada(page, 5).getByTestId("history-acao-ok").count(), 0, "a mensagem da ação anterior não sobra");
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl history undo 5";
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  const item = daEntrada(page, 5);
  await item.getByTestId("history-desfazer").click();
  await item.getByTestId("history-comando").waitFor();
  assert.equal(await item.getByTestId("history-comando").innerText(), comando);
  assert.match(await item.getByTestId("history-sem-terminal").innerText(), /nenhum terminal/i);
  assert.equal(await item.getByTestId("history-acao-ok").count(), 0);
  assert.equal(await item.getByTestId("history-desfazer").isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("Copiar põe o comando na área de transferência", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: COMANDO } }) });
  await abrirAba(page);
  await page.evaluate(() => {
    Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (t) => void (window.__copiado = t) } });
  });
  const item = daEntrada(page, 5);
  await item.getByTestId("history-desfazer").click();
  await item.getByTestId("history-copiar").click();
  await page.waitForFunction(() => window.__copiado !== undefined);
  assert.equal(await page.evaluate(() => window.__copiado), COMANDO);
  await close();
});

test("erro do backend aparece na entrada, com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "invalid_terminal", message: "NODUS_TERMINAL precisa ser o caminho absoluto de um terminal conhecido" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  const item = daEntrada(page, 5);
  await item.getByTestId("history-desfazer").click();
  await item.getByTestId("history-acao-erro").waitFor();
  assert.match(await item.getByTestId("history-acao-erro").innerText(), /NODUS_TERMINAL/);
  assert.equal(await item.getByTestId("history-desfazer").isDisabled(), false);
  assert.equal(await page.getByTestId("history-list").count(), 1, "o histórico continua na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  const item = daEntrada(page, 5);
  await item.getByTestId("history-desfazer").click();
  await item.getByTestId("history-comando").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await item.getByTestId("history-comando").innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await daEntrada(page, 5).getByTestId("history-desfazer").click();
  await daEntrada(page, 5).getByTestId("history-acao-ok").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
