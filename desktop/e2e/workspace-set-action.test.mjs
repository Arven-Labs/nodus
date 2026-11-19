// A ação "Usar no terminal" nos candidatos da aba Workspace num navegador, com o backend de mentira (ADR 0028). É a primeira ação
// em que a tela manda um caminho: prova que o botão só existe em candidato que a CLI listou, que o clique manda o NOME da ação e
// EXATAMENTE o caminho do candidato clicado (nada digitado, nada montado), que o texto não promete uma confirmação (o `workspace set`
// da CLI não pergunta), que não abre duas janelas por clique duplo, que "Atualizar" relê o workspace, o fallback, o erro e que dado
// hostil (nome de pasta, comando) vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const workspaceText = readFileSync(new URL("../tests/fixtures/workspace.json", import.meta.url), "utf8");
const workspaceFixture = () => JSON.parse(workspaceText);
const detectado = workspaceText;
const WS = "/home/exemplo/ws";
const PROJECTS = "/home/exemplo/Projects";

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

const COMANDO = `/usr/bin/nodusctl workspace set -- ${PROJECTS}`;

/** `reads`: o que cada chamada sucessiva de `workspace` devolve (a última se repete). `action`: o que `run_action` devolve. */
const handlers = ({ reads = [detectado], action = { launched: true, command: COMANDO } } = {}) => {
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

test("há candidatos: um botão por candidato, e ainda nada foi lançado", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  assert.equal(await page.getByTestId("workspace-usar").count(), 2);
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("sem candidatos (escolha gravada, ou nenhum workspace) não há botão de usar", async () => {
  const gravado = { ...workspaceFixture(), chosen: { path: WS, source: "stored", repo_count: 3 }, candidates: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(gravado)] }) });
  await abrirAba(page);
  assert.equal(await page.getByTestId("workspace-usar").count(), 0);
  await close();

  const nenhum = JSON.stringify({ chosen: null, candidates: [] });
  const outra = await openApp(browser, app, { handlers: handlers({ reads: [nenhum] }) });
  await outra.page.getByTestId("system").waitFor();
  await outra.page.getByTestId("tab-workspace").click();
  await outra.page.getByTestId("workspace-nenhum").waitFor();
  assert.equal(await outra.page.getByTestId("workspace-usar").count(), 0);
  await outra.close();
});

test("o clique manda o nome da ação e só o caminho do candidato clicado", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).click();
  await page.getByTestId("workspace-usar-acao-ok").waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "workspace_set", path: PROJECTS }]);
  assert.deepEqual(errors, []);
  await close();

  const primeiro = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(primeiro.page);
  await primeiro.page.getByTestId("workspace-usar").nth(0).click();
  await primeiro.page.getByTestId("workspace-usar-acao-ok").waitFor();
  assert.deepEqual(acoes(primeiro.calls).map((c) => c.args), [{ command: "workspace_set", path: WS }]);
  await primeiro.close();
});

test("terminal aberto: não promete confirmação (a CLI não pergunta), e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).click();
  const texto = await page.getByTestId("workspace-usar-acao-ok").innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /atualizar/i);
  assert.doesNotMatch(texto, /confirme|pergunta/i, "o `workspace set` não pergunta: o texto não pode dizer que sim");
  assert.equal(await page.getByTestId("workspace-usar").nth(1).isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).dblclick();
  await page.getByTestId("workspace-usar-acao-ok").waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê o workspace, e a escolha nova aparece como gravada", async () => {
  const depois = { chosen: { path: PROJECTS, source: "stored", repo_count: 0 }, candidates: [] };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ reads: [detectado, JSON.stringify(depois)] }) });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).click();
  await page.getByTestId("workspace-usar-atualizar").click();
  await page.getByTestId("workspace-esquecer").waitFor();
  assert.deepEqual(leituras(calls), ["inventory", "workspace", "workspace"]);
  assert.match(await page.getByTestId("workspace-escolhido").innerText(), new RegExp(PROJECTS));
  assert.equal(await page.getByTestId("workspace-usar").count(), 0, "sem candidatos, sem botão");
  assert.equal(await page.getByTestId("workspace-usar-acao-ok").count(), 0, "a mensagem da ação anterior não sobra");
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = `nodusctl workspace set -- '/home/exemplo/meus projetos'`;
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).click();
  await page.getByTestId("workspace-usar-comando").waitFor();
  assert.equal(await page.getByTestId("workspace-usar-comando").innerText(), comando);
  assert.match(await page.getByTestId("workspace-usar-sem-terminal").innerText(), /nenhum terminal/i);
  assert.equal(await page.getByTestId("workspace-usar-acao-ok").count(), 0);
  assert.equal(await page.getByTestId("workspace-usar").nth(1).isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("erro do backend (por exemplo, o caminho saiu da lista) aparece com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "not_in_list", message: "o caminho não está entre os candidatos que o nodusctl lista agora" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).click();
  await page.getByTestId("workspace-usar-acao-erro").waitFor();
  assert.match(await page.getByTestId("workspace-usar-acao-erro").innerText(), /candidatos/);
  assert.equal(await page.getByTestId("workspace-usar").nth(1).isDisabled(), false);
  assert.equal(await page.getByTestId("workspace-escolhido").count(), 1, "o workspace continua na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("nome de pasta hostil vira texto na tela e vai como dado, exatamente igual, na ação", async () => {
  const hostil = `/home/exemplo/<img src=x onerror="window.__pwned=1">'; touch PWNED; '`;
  const lista = { chosen: null, candidates: [{ path: hostil, repo_count: 1 }] };
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-workspace").click();
  await page.getByTestId("workspace-candidato").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("workspace-candidato").innerText(), /touch PWNED/);
  await page.getByTestId("workspace-usar").click();
  await page.getByTestId("workspace-usar-acao-ok").waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "workspace_set", path: hostil }]);
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil devolvido pelo backend vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).click();
  await page.getByTestId("workspace-usar-comando").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await page.getByTestId("workspace-usar-comando").innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("workspace-usar").nth(1).click();
  await page.getByTestId("workspace-usar-acao-ok").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
