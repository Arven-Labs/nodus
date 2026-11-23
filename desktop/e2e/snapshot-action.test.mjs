// A ação "Gravar snapshot no terminal" na aba Compare num navegador, com o backend de mentira (ADR 0027, ADR 0032). Prova que há UM
// botão, visível com pares e sem pares (o estado vazio é justamente quando se precisa gravar), que o clique manda só o NOME da ação
// (sem rótulo, sem caminho, sem número: o rótulo é o hostname, decidido pela CLI), que o texto NÃO manda confirmar nada (o `snapshot`
// não pergunta), que não abre duas janelas por clique duplo, que "Atualizar" relê a comparação, o fallback, o erro e que comando
// hostil vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const compareText = readFileSync(new URL("../tests/fixtures/compare_all.json", import.meta.url), "utf8");
const vazioText = JSON.stringify({ comparisons: [], unreadable: [] });

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

const COMANDO = "/usr/bin/nodusctl snapshot";
const BOTAO = "compare-gravar";

/** `reads`: o que cada chamada sucessiva de `compare_all` devolve (a última se repete). `action`: o que `run_action` devolve. */
const handlers = ({ reads = [compareText], action = { launched: true, command: COMANDO } } = {}) => {
  let n = 0;
  return {
    read_nodusctl: ({ command }) => {
      if (command === "inventory") return inventoryText;
      if (command === "compare_all") return reads[Math.min(n++, reads.length - 1)];
      return { __error: { kind: "not_allowed", message: command } };
    },
    run_action: typeof action === "function" ? action : () => action,
  };
};
const leituras = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);
const acoes = (calls) => calls.filter((c) => c.cmd === "run_action");

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-compare").click();
  await page.getByTestId("compare-nota").waitFor();
}

test("com pares há um botão só, e ainda nada foi lançado", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  assert.ok((await page.getByTestId("compare-par").count()) >= 1);
  assert.equal(await page.getByTestId(BOTAO).count(), 1);
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("sem nenhum par (estado vazio) o botão continua lá: é quando se precisa gravar", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: [vazioText] }) });
  await abrirAba(page);
  await page.getByTestId("compare-vazio").waitFor();
  assert.equal(await page.getByTestId(BOTAO).count(), 1);
  await close();
});

test("o clique manda só o nome da ação: sem rótulo, sem caminho e sem número", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-acao-ok`).waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "snapshot" }]);
  assert.deepEqual(errors, []);
  await close();
});

test("terminal aberto: o texto não manda confirmar nada, e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  const texto = await page.getByTestId(`${BOTAO}-acao-ok`).innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /atualizar/i);
  assert.doesNotMatch(texto, /confirme|pergunta/i, "o `snapshot` não pergunta nada");
  assert.equal(await page.getByTestId(BOTAO).isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await page.getByTestId(BOTAO).dblclick();
  await page.getByTestId(`${BOTAO}-acao-ok`).waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê a comparação: o estado vazio dá lugar aos pares", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ reads: [vazioText, compareText] }) });
  await abrirAba(page);
  await page.getByTestId("compare-vazio").waitFor();
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-atualizar`).click();
  await page.getByTestId("compare-par").first().waitFor();
  assert.equal(await page.getByTestId("compare-vazio").count(), 0);
  assert.deepEqual(leituras(calls), ["inventory", "compare_all", "compare_all"]);
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl snapshot";
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-comando`).waitFor();
  assert.equal(await page.getByTestId(`${BOTAO}-comando`).innerText(), comando);
  assert.match(await page.getByTestId(`${BOTAO}-sem-terminal`).innerText(), /nenhum terminal/i);
  assert.equal(await page.getByTestId(`${BOTAO}-acao-ok`).count(), 0);
  assert.equal(await page.getByTestId(BOTAO).isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("erro do backend aparece com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "version_mismatch", message: "o nodusctl é de outra versão" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-acao-erro`).waitFor();
  assert.match(await page.getByTestId(`${BOTAO}-acao-erro`).innerText(), /outra versão/);
  assert.equal(await page.getByTestId(BOTAO).isDisabled(), false);
  assert.ok((await page.getByTestId("compare-par").count()) >= 1, "a comparação continua na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil devolvido pelo backend vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-comando`).waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await page.getByTestId(`${BOTAO}-comando`).innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-acao-ok`).waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
