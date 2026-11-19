// A ação "Esquecer no terminal" da aba Workspace num navegador, com o backend de mentira (ADR 0027). Prova que o botão só aparece
// quando há uma escolha gravada com `workspace set` (origem `stored`, o único caso em que o `unset` tem o que apagar), que o clique
// manda só o NOME da ação (sem caminho nem argumento), que o texto NÃO promete uma confirmação (o `workspace unset` da CLI não
// pergunta nada), que não abre duas janelas por clique duplo, que "Atualizar" relê o workspace, o fallback com o comando, o erro e
// que dado hostil vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const workspaceText = readFileSync(new URL("../tests/fixtures/workspace.json", import.meta.url), "utf8");
const workspaceFixture = () => JSON.parse(workspaceText);
/** O workspace com a origem dada (a fixture padrão é `detected`). */
const comOrigem = (source) => {
  const w = workspaceFixture();
  w.chosen.source = source;
  return JSON.stringify(w);
};
const gravado = comOrigem("stored");

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

const COMANDO = "/usr/bin/nodusctl workspace unset";

/** `reads`: o que cada chamada sucessiva de `workspace` devolve (a última se repete). `action`: o que `run_action` devolve. */
const handlers = ({ reads = [gravado], action = { launched: true, command: COMANDO } } = {}) => {
  let n = 0;
  return {
    read_nodusctl: ({ command }) => {
      if (command === "inventory") return inventoryText;
      if (command === "workspace") return reads[Math.min(n++, reads.length - 1)];
      return { __error: { kind: "not_allowed", message: command } };
    },
    run_action: typeof action === "function" ? action : () => action,
  };
};
const leituras = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);
const acoes = (calls) => calls.filter((c) => c.cmd === "run_action");

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-workspace").click();
  await page.getByTestId("workspace-escolhido").waitFor();
}

test("há escolha gravada: o botão aparece e ainda nada foi lançado", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").waitFor();
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("o botão só aparece quando a origem é a escolha gravada", async () => {
  const casos = [
    ["gravado com `workspace set`", gravado, true],
    ["detectado sob o HOME", comOrigem("detected"), false],
    ["variável NODUS_WORKSPACE (o `unset` não a muda)", comOrigem("environment"), false],
    ["informado por argumento", comOrigem("informed"), false],
  ];
  for (const [nome, leitura, aparece] of casos) {
    const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: [leitura] }) });
    await abrirAba(page);
    assert.equal(await page.getByTestId("workspace-esquecer").count(), aparece ? 1 : 0, nome);
    await close();
  }
  const semEscolha = JSON.stringify({ chosen: null, candidates: [] });
  const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: [semEscolha] }) });
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-workspace").click();
  await page.getByTestId("workspace-nenhum").waitFor();
  assert.equal(await page.getByTestId("workspace-esquecer").count(), 0, "nenhum workspace");
  await close();
});

test("o clique manda só o nome da ação: nenhum caminho, comando ou argumento", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").click();
  await page.getByTestId("workspace-acao-ok").waitFor();
  const feitas = acoes(calls);
  assert.equal(feitas.length, 1);
  assert.deepEqual(feitas[0].args, { command: "workspace_unset" });
  assert.deepEqual(errors, []);
  await close();
});

test("terminal aberto: não promete confirmação (a CLI não pergunta), e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").click();
  const texto = await page.getByTestId("workspace-acao-ok").innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /atualizar/i);
  assert.doesNotMatch(texto, /confirme|pergunta/i, "o `workspace unset` não pergunta: o texto não pode dizer que sim");
  assert.equal(await page.getByTestId("workspace-esquecer").isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").dblclick();
  await page.getByTestId("workspace-acao-ok").waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê o workspace, e a escolha esquecida muda na tela", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ reads: [gravado, comOrigem("detected")] }) });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").click();
  await page.getByTestId("workspace-atualizar").click();
  await page.getByTestId("workspace-escolhido").waitFor();
  assert.deepEqual(leituras(calls), ["inventory", "workspace", "workspace"]);
  assert.match(await page.getByTestId("workspace-escolhido").innerText(), /detectado/i);
  assert.equal(await page.getByTestId("workspace-esquecer").count(), 0);
  assert.equal(await page.getByTestId("workspace-acao-ok").count(), 0, "a mensagem da ação anterior não sobra");
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl workspace unset";
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").click();
  await page.getByTestId("workspace-comando").waitFor();
  assert.equal(await page.getByTestId("workspace-comando").innerText(), comando);
  assert.match(await page.getByTestId("workspace-sem-terminal").innerText(), /nenhum terminal/i);
  assert.equal(await page.getByTestId("workspace-acao-ok").count(), 0);
  assert.equal(await page.getByTestId("workspace-esquecer").isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("erro do backend aparece com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "invalid_terminal", message: "NODUS_TERMINAL precisa ser o caminho absoluto de um terminal conhecido" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").click();
  await page.getByTestId("workspace-acao-erro").waitFor();
  assert.match(await page.getByTestId("workspace-acao-erro").innerText(), /NODUS_TERMINAL/);
  assert.equal(await page.getByTestId("workspace-esquecer").isDisabled(), false);
  assert.equal(await page.getByTestId("workspace-escolhido").count(), 1, "o workspace continua na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").click();
  await page.getByTestId("workspace-comando").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await page.getByTestId("workspace-comando").innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a nota da aba diz que a janela não esquece nada e que quem roda é o terminal", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const texto = await page.getByTestId("workspace-nota").innerText();
  assert.match(texto, /não esquece nada/i);
  assert.match(texto, /abre um terminal/i);
  assert.match(texto, /workspace unset/);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-esquecer").click();
  await page.getByTestId("workspace-acao-ok").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
