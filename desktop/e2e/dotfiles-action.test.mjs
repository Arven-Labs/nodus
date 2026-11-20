// A ação "Adotar no terminal" nas linhas da aba Dotfiles num navegador, com o backend de mentira (ADR 0028). Prova que o botão só
// existe em arquivo comum que a CLI listou e que cabe no limite de 1 MiB do `adopt` (symlink, ausente, pasta e outro tipo não têm o
// que adotar), que o clique manda o NOME da ação e EXATAMENTE o caminho da linha clicada, que o texto manda confirmar no terminal
// (o `adopt` mostra o plano e pergunta `[s/N]`), que não abre duas janelas por clique duplo, que "Atualizar" relê os dotfiles, o
// fallback, o erro e que dado hostil (nome de arquivo, comando) vira texto. Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const dotfilesText = readFileSync(new URL("../tests/fixtures/dotfiles.json", import.meta.url), "utf8");
const dotfilesFixture = () => JSON.parse(dotfilesText);
const MIB = 1 << 20;

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

const COMANDO = "/usr/bin/nodusctl dotfiles adopt -- .bashrc";

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

test("a lista de exemplo tem um arquivo comum: um botão, e ainda nada foi lançado", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  assert.equal(await page.getByTestId("dotfile-adotar").count(), 1);
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("o botão só aparece em arquivo comum que cabe no limite do `adopt`", async () => {
  const lista = [
    { path: ".um", kind: "file", size: 0 },
    { path: ".limite", kind: "file", size: MIB },
    { path: ".grande", kind: "file", size: MIB + 1 },
    { path: ".link", kind: "symlink", target: "/dotfiles/.link", outside_home: true },
    { path: ".falta", kind: "missing" },
    { path: ".pasta", kind: "dir" },
    { path: ".outro", kind: "other" },
  ];
  const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await abrirAba(page);
  const linhas = page.getByTestId("dotfile");
  assert.equal(await linhas.count(), 7);
  const comBotao = [];
  for (let i = 0; i < 7; i++) {
    const linha = linhas.nth(i);
    if ((await linha.getByTestId("dotfile-adotar").count()) === 1) comBotao.push((await linha.locator("strong").innerText()).trim());
  }
  assert.deepEqual(comBotao, [".um", ".limite"], ".grande (1 MiB + 1) a CLI recusa; os outros tipos não têm o que adotar");
  await close();
});

test("sem nenhum arquivo comum na lista, não há botão", async () => {
  const lista = dotfilesFixture().filter((e) => e.kind !== "file");
  const { page, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await abrirAba(page);
  assert.equal(await page.getByTestId("dotfile-adotar").count(), 0);
  await close();
});

test("o clique manda o nome da ação e só o caminho da linha clicada", async () => {
  const lista = [...dotfilesFixture(), { path: ".config/git/config", kind: "file", size: 80 }];
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").nth(1).click();
  await page.getByTestId("dotfile-adotar-acao-ok").waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "dotfiles_adopt", path: ".config/git/config" }]);
  assert.deepEqual(errors, []);
  await close();

  const primeiro = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await abrirAba(primeiro.page);
  await primeiro.page.getByTestId("dotfile-adotar").nth(0).click();
  await primeiro.page.getByTestId("dotfile-adotar-acao-ok").waitFor();
  assert.deepEqual(acoes(primeiro.calls).map((c) => c.args), [{ command: "dotfiles_adopt", path: ".bashrc" }]);
  await primeiro.close();
});

test("terminal aberto: manda confirmar lá (o `adopt` mostra o plano e pergunta), e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").click();
  const texto = await page.getByTestId("dotfile-adotar-acao-ok").innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /confirme a adoção/i);
  assert.match(texto, /atualizar/i);
  assert.equal(await page.getByTestId("dotfile-adotar").isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").dblclick();
  await page.getByTestId("dotfile-adotar-acao-ok").waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê os dotfiles, e o arquivo adotado vira symlink e perde o botão", async () => {
  const depois = dotfilesFixture().map((e) =>
    e.path === ".bashrc" ? { path: ".bashrc", kind: "symlink", target: "/dotfiles/.bashrc", outside_home: true } : e,
  );
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ reads: [dotfilesText, JSON.stringify(depois)] }) });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").click();
  await page.getByTestId("dotfile-adotar-atualizar").click();
  await page.getByTestId("dotfiles-list").waitFor();
  await page.waitForFunction(() => document.querySelectorAll('[data-testid="dotfile-adotar"]').length === 0);
  assert.deepEqual(leituras(calls), ["inventory", "dotfiles", "dotfiles"]);
  assert.equal(await page.getByTestId("dotfile-adotar-acao-ok").count(), 0, "a mensagem da ação anterior não sobra");
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl dotfiles adopt -- '.config/meu app/conf'";
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").click();
  await page.getByTestId("dotfile-adotar-comando").waitFor();
  assert.equal(await page.getByTestId("dotfile-adotar-comando").innerText(), comando);
  assert.match(await page.getByTestId("dotfile-adotar-sem-terminal").innerText(), /nenhum terminal/i);
  assert.equal(await page.getByTestId("dotfile-adotar-acao-ok").count(), 0);
  assert.equal(await page.getByTestId("dotfile-adotar").isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("erro do backend (por exemplo, o arquivo saiu da lista) aparece com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "not_in_list", message: "o caminho não está entre os itens que o nodusctl lista agora" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").click();
  await page.getByTestId("dotfile-adotar-acao-erro").waitFor();
  assert.match(await page.getByTestId("dotfile-adotar-acao-erro").innerText(), /itens/);
  assert.equal(await page.getByTestId("dotfile-adotar").isDisabled(), false);
  assert.equal(await page.getByTestId("dotfiles-list").count(), 1, "a lista continua na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("nome de arquivo hostil vira texto na tela e vai como dado, exatamente igual, na ação", async () => {
  const hostil = `.<img src=x onerror="window.__pwned=1">'; touch PWNED; '`;
  const lista = [{ path: hostil, kind: "file", size: 10 }];
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers({ reads: [JSON.stringify(lista)] }) });
  await abrirAba(page);
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("dotfile").innerText(), /touch PWNED/);
  await page.getByTestId("dotfile-adotar").click();
  await page.getByTestId("dotfile-adotar-acao-ok").waitFor();
  assert.deepEqual(acoes(calls).map((c) => c.args), [{ command: "dotfiles_adopt", path: hostil }]);
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil devolvido pelo backend vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").click();
  await page.getByTestId("dotfile-adotar-comando").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await page.getByTestId("dotfile-adotar-comando").innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("dotfile-adotar").click();
  await page.getByTestId("dotfile-adotar-acao-ok").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
