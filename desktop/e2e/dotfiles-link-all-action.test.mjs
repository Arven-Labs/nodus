// A ação "Ligar todos no terminal" na aba Dotfiles num navegador, com o backend de mentira (ADR 0027, ADR 0030). Prova que há UM
// botão de lote (não um por linha), que ele só existe quando a CLI listou ao menos uma entrada ausente do HOME, que o clique manda
// só o NOME da ação (sem caminho e sem número: o lote é a lista curada, decidida pela CLI), que o texto manda confirmar no terminal
// (o `link --all` mostra um plano e pergunta `[s/N]`), que não abre duas janelas por clique duplo, que "Atualizar" relê os dotfiles,
// o fallback, o erro e que comando hostil vira texto. Dados inventados.

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

const COMANDO = "/usr/bin/nodusctl dotfiles link --all";
const BOTAO = "dotfiles-ligar-todos";

/** `reads`: o que cada chamada sucessiva de `dotfiles` devolve (a última se repete). `action`: o que `run_action` devolve. */
const handlers = ({ reads = [dotfilesText], action = { launched: true, command: COMANDO } } = {}) => {
  let n = 0;
  return {
    read_nodusctl: ({ command }) => {
      if (command === "inventory") return inventoryText;
      if (command === "dotfiles") return reads[Math.min(n++, reads.length - 1)];
      return { __error: { kind: "not_allowed", message: command } };
    },
    run_action: typeof action === "function" ? action : () => action,
  };
};
const leituras = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);
const acoes = (calls) => calls.filter((c) => c.cmd === "run_action");

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-dotfiles").click();
  await page.getByTestId("dotfiles-list").waitFor();
}

test("com entradas ausentes há um botão de lote só (mesmo com várias ausentes), e ainda nada foi lançado", async () => {
  const lista = [...dotfilesFixture(), { path: ".config/nvim/init.lua", kind: "missing" }];
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await abrirAba(page);
  assert.ok((await page.getByTestId("dotfile-ligar").count()) >= 2, "a lista de teste tem duas ausentes");
  assert.equal(await page.getByTestId(BOTAO).count(), 1);
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("sem nenhuma entrada ausente na lista, não há botão de lote", async () => {
  const lista = dotfilesFixture().filter((e) => e.kind !== "missing");
  const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await abrirAba(page);
  assert.equal(await page.getByTestId(BOTAO).count(), 0);
  await close();
});

test("lista vazia: sem botão de lote", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: ["[]"] }) });
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-dotfiles").click();
  await page.getByTestId("dotfiles-vazio").waitFor();
  assert.equal(await page.getByTestId(BOTAO).count(), 0);
  await close();
});

test("o clique manda só o nome da ação: sem caminho e sem número", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-acao-ok`).waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "dotfiles_link_all" }]);
  assert.deepEqual(errors, []);
  await close();
});

test("terminal aberto: manda confirmar lá (o `link --all` mostra o plano e pergunta), e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  const texto = await page.getByTestId(`${BOTAO}-acao-ok`).innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /confirme a ligação/i);
  assert.match(texto, /atualizar/i);
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

test("'Atualizar' relê os dotfiles, e sem mais nenhuma ausente o botão de lote some", async () => {
  const depois = dotfilesFixture().map((e) =>
    e.kind === "missing" ? { path: e.path, kind: "symlink", target: `/dotfiles/${e.path}`, outside_home: true } : e,
  );
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ reads: [dotfilesText, JSON.stringify(depois)] }) });
  await abrirAba(page);
  await page.getByTestId(BOTAO).click();
  await page.getByTestId(`${BOTAO}-atualizar`).click();
  await page.getByTestId("dotfiles-list").waitFor();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid="dotfiles-ligar-todos"]').length === 0);
  assert.deepEqual(leituras(calls), ["inventory", "dotfiles", "dotfiles"]);
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl dotfiles link --all";
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
  assert.equal(await page.getByTestId("dotfiles-list").count(), 1, "a lista continua na tela");
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
