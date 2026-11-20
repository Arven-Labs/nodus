// A ação "Voltar a sugerir no terminal" nos itens ignorados da aba Sugestões num navegador, com o backend de mentira (ADR 0028).
// Prova que há um botão por nome que a CLI listou em `ignored`, que o clique manda o NOME da ação e EXATAMENTE o nome do item
// clicado (nada digitado, nada montado), que o texto não promete confirmação (o `preset unignore` da CLI não pergunta), que não abre
// duas janelas por clique duplo, que "Atualizar" relê as sugestões, o fallback, o erro e que dado hostil (nome, comando) vira texto.
// Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const suggestText = readFileSync(new URL("../tests/fixtures/preset_suggest.json", import.meta.url), "utf8");
const suggestFixture = () => JSON.parse(suggestText);

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

const COMANDO = "/usr/bin/nodusctl preset unignore -- sl";

/** `suggests`: o que cada chamada sucessiva de `preset_suggest` devolve (a última se repete). `action`: o que `run_action` devolve. */
const handlers = ({ suggests = [suggestText], action = { launched: true, command: COMANDO } } = {}) => {
  let n = 0;
  return {
    read_nodusctl: ({ command }) => {
      if (command === "inventory") return inventoryText;
      if (command === "preset_suggest") return suggests[Math.min(n++, suggests.length - 1)];
      return { __error: { kind: "not_allowed", message: command } };
    },
    run_action: typeof action === "function" ? action : () => action,
  };
};
const leituras = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);
const acoes = (calls) => calls.filter((c) => c.cmd === "run_action");

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-suggest").click();
  await page.getByTestId("suggest-pacman").waitFor();
}

test("há ignorados: o nome de cada um e um botão por nome, e ainda nada foi lançado", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  assert.equal(await page.getByTestId("suggest-ignorado").count(), 2);
  assert.match(await page.getByTestId("suggest-ignorado").nth(0).innerText(), /cmatrix/);
  assert.match(await page.getByTestId("suggest-ignorado").nth(1).innerText(), /sl/);
  assert.equal(await page.getByTestId("suggest-voltar").count(), 2);
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("sem ignorados não há botão de voltar a sugerir", async () => {
  const sem = { ...suggestFixture(), ignored: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers({ suggests: [JSON.stringify(sem)] }) });
  await abrirAba(page);
  assert.equal(await page.getByTestId("suggest-ignorado").count(), 0);
  assert.equal(await page.getByTestId("suggest-voltar").count(), 0);
  await close();
});

test("o botão aparece mesmo quando só há ignorados (nada a adotar)", async () => {
  const soIgnorados = { ...suggestFixture(), pacman_extra: [], foreign_extra: [], flatpak_extra: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers({ suggests: [JSON.stringify(soIgnorados)] }) });
  await abrirAba(page);
  assert.equal(await page.getByTestId("suggest-voltar").count(), 2);
  assert.equal(await page.getByTestId("suggest-adotar").count(), 0);
  await close();
});

test("o clique manda o nome da ação e só o nome do item clicado", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).click();
  await page.getByTestId("suggest-voltar-acao-ok").waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "preset_unignore", path: "sl" }]);
  assert.deepEqual(errors, []);
  await close();

  const primeiro = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(primeiro.page);
  await primeiro.page.getByTestId("suggest-voltar").nth(0).click();
  await primeiro.page.getByTestId("suggest-voltar-acao-ok").waitFor();
  assert.deepEqual(acoes(primeiro.calls).map((c) => c.args), [{ command: "preset_unignore", path: "cmatrix" }]);
  await primeiro.close();
});

test("terminal aberto: não promete confirmação (a CLI não pergunta), e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).click();
  const texto = await page.getByTestId("suggest-voltar-acao-ok").innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /atualizar/i);
  assert.doesNotMatch(texto, /confirme|pergunta/i, "o `preset unignore` não pergunta: o texto não pode dizer que sim");
  assert.equal(await page.getByTestId("suggest-voltar").nth(1).isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).dblclick();
  await page.getByTestId("suggest-voltar-acao-ok").waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê as sugestões, e o item que voltou sai da lista de ignorados", async () => {
  const depois = { ...suggestFixture(), pacman_extra: ["neovim", "sl", "tmux"], ignored: ["cmatrix"] };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ suggests: [suggestText, JSON.stringify(depois)] }) });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).click();
  await page.getByTestId("suggest-voltar-atualizar").click();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid="suggest-ignorado"]').length === 1);
  assert.deepEqual(leituras(calls), ["inventory", "preset_suggest", "preset_suggest"]);
  assert.match(await page.getByTestId("suggest-ignorado").innerText(), /cmatrix/);
  assert.equal(await page.getByTestId("suggest-voltar-acao-ok").count(), 0, "a mensagem da ação anterior não sobra");
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl preset unignore -- sl";
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).click();
  await page.getByTestId("suggest-voltar-comando").waitFor();
  assert.equal(await page.getByTestId("suggest-voltar-comando").innerText(), comando);
  assert.match(await page.getByTestId("suggest-voltar-sem-terminal").innerText(), /nenhum terminal/i);
  assert.equal(await page.getByTestId("suggest-voltar-acao-ok").count(), 0);
  assert.equal(await page.getByTestId("suggest-voltar").nth(1).isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("erro do backend (por exemplo, o nome saiu da lista) aparece com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "not_in_list", message: "o nome não está entre os itens que o nodusctl lista agora" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).click();
  await page.getByTestId("suggest-voltar-acao-erro").waitFor();
  assert.match(await page.getByTestId("suggest-voltar-acao-erro").innerText(), /itens/);
  assert.equal(await page.getByTestId("suggest-voltar").nth(1).isDisabled(), false);
  assert.equal(await page.getByTestId("suggest-pacman").count(), 1, "as sugestões continuam na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("nome ignorado hostil vira texto na tela e vai como dado, exatamente igual, na ação", async () => {
  const hostil = `<img src=x onerror="window.__pwned=1">'; touch PWNED; '`;
  const sugestoes = { ...suggestFixture(), ignored: [hostil] };
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers({ suggests: [JSON.stringify(sugestoes)] }) });
  await abrirAba(page);
  await page.getByTestId("suggest-ignorado").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("suggest-ignorado").innerText(), /touch PWNED/);
  await page.getByTestId("suggest-voltar").click();
  await page.getByTestId("suggest-voltar-acao-ok").waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "preset_unignore", path: hostil }]);
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil devolvido pelo backend vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).click();
  await page.getByTestId("suggest-voltar-comando").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await page.getByTestId("suggest-voltar-comando").innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-voltar").nth(1).click();
  await page.getByTestId("suggest-voltar-acao-ok").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
