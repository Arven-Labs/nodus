// A ação "Adotar no terminal" da aba Sugestões num navegador, com o backend de mentira (ADR 0027). Prova que o botão só aparece
// quando há algo que o `preset adopt` perguntaria (pacman ou Flatpak fora do preset; os "de fora dos repositórios" ele não
// pergunta), que o clique manda só o NOME da ação (o terminal, o caminho do preset e as respostas `s/N/i/q` são do Rust e da CLI),
// que não abre duas janelas por clique duplo, que "Atualizar" relê as sugestões, o fallback com o comando, o erro e que dado
// hostil vira texto. Dados inventados.

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

const COMANDO = "/usr/bin/nodusctl preset adopt /tmp/preset.toml";

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

test("há o que adotar: o botão aparece e ainda nada foi lançado", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").waitFor();
  assert.equal(acoes(calls).length, 0, "só o clique lança");
  await close();
});

test("o botão só aparece quando o `adopt` perguntaria algo", async () => {
  const vazio = { ...suggestFixture(), pacman_extra: [], foreign_extra: [], flatpak_extra: [] };
  const casos = [
    ["nada fora do preset", vazio, false],
    ["só de fora dos repositórios (o `adopt` não pergunta)", { ...vazio, foreign_extra: ["brave-bin"] }, false],
    ["só pacman", { ...vazio, pacman_extra: ["neovim"] }, true],
    ["só Flatpak", { ...vazio, flatpak_extra: ["org.mozilla.firefox"] }, true],
    ["só ignorados pelo config", { ...vazio, ignored: ["cmatrix"] }, false],
  ];
  for (const [nome, sugestoes, aparece] of casos) {
    const { page, close } = await openApp(browser, app, { handlers: handlers({ suggests: [JSON.stringify(sugestoes)] }) });
    await abrirAba(page);
    assert.equal(await page.getByTestId("suggest-adotar").count(), aparece ? 1 : 0, nome);
    await close();
  }
});

test("o clique manda só o nome da ação: nenhum caminho, comando, argumento ou resposta", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").click();
  await page.getByTestId("suggest-acao-ok").waitFor();
  const feitas = acoes(calls);
  assert.equal(feitas.length, 1);
  assert.deepEqual(feitas[0].args, { command: "preset_adopt" });
  assert.deepEqual(errors, []);
  await close();
});

test("terminal aberto: diz para responder lá e o botão fica desabilitado até 'Atualizar'", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").click();
  const texto = await page.getByTestId("suggest-acao-ok").innerText();
  assert.match(texto, /terminal/i);
  assert.match(texto, /confirme a adoção/i);
  assert.match(texto, /atualizar/i);
  assert.equal(await page.getByTestId("suggest-adotar").isDisabled(), true);
  await close();
});

test("clique duplo abre uma janela só", async () => {
  const lento = async () => {
    await new Promise((resolve) => setTimeout(resolve, 400));
    return { launched: true, command: COMANDO };
  };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ action: lento }) });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").dblclick();
  await page.getByTestId("suggest-acao-ok").waitFor();
  assert.equal(acoes(calls).length, 1);
  await close();
});

test("'Atualizar' relê as sugestões, e o que foi adotado some da tela", async () => {
  const depois = { ...suggestFixture(), pacman_extra: [], foreign_extra: [], flatpak_extra: [] };
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers({ suggests: [suggestText, JSON.stringify(depois)] }) });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").click();
  await page.getByTestId("suggest-atualizar").click();
  await page.getByTestId("suggest-nada").waitFor();
  assert.deepEqual(leituras(calls), ["inventory", "preset_suggest", "preset_suggest"]);
  assert.equal(await page.getByTestId("suggest-adotar").count(), 0);
  assert.equal(await page.getByTestId("suggest-acao-ok").count(), 0, "a mensagem da ação anterior não sobra");
  await close();
});

test("sem terminal: mostra o comando exato como texto, com Copiar, e não diz que abriu nada", async () => {
  const comando = "nodusctl preset adopt '/tmp/minha pasta/p.toml'";
  const { page, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: comando } }) });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").click();
  await page.getByTestId("suggest-comando").waitFor();
  assert.equal(await page.getByTestId("suggest-comando").innerText(), comando);
  assert.match(await page.getByTestId("suggest-sem-terminal").innerText(), /nenhum terminal/i);
  assert.equal(await page.getByTestId("suggest-acao-ok").count(), 0);
  assert.equal(await page.getByTestId("suggest-adotar").isDisabled(), false, "nada foi aberto: pode tentar de novo");
  await close();
});

test("erro do backend aparece com a mensagem dele, e o botão volta a poder ser usado", async () => {
  const erro = { __error: { kind: "preset_not_set", message: "defina NODUS_PRESET com o caminho absoluto do arquivo de preset" } };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: erro }) });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").click();
  await page.getByTestId("suggest-acao-erro").waitFor();
  assert.match(await page.getByTestId("suggest-acao-erro").innerText(), /NODUS_PRESET/);
  assert.equal(await page.getByTestId("suggest-adotar").isDisabled(), false);
  assert.equal(await page.getByTestId("suggest-pacman").count(), 1, "a lista continua na tela");
  assert.deepEqual(errors, []);
  await close();
});

test("comando hostil vira texto: nada é executado nem vira elemento", async () => {
  const hostil = '<img src=x onerror="window.__pwned=1">';
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers({ action: { launched: false, command: hostil } }) });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").click();
  await page.getByTestId("suggest-comando").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.equal(await page.getByTestId("suggest-comando").innerText(), hostil);
  assert.deepEqual(errors, []);
  await close();
});

test("a nota da aba diz que a janela não adota nada e que a pergunta item a item é no terminal", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const texto = await page.getByTestId("suggest-nota").innerText();
  assert.match(texto, /não adota nada/i);
  assert.match(texto, /abre um terminal/i);
  assert.match(texto, /preset adopt/);
  assert.match(texto, /item a item/i);
  await close();
});

test("a tela só fala com o backend por `read_nodusctl` e `run_action`", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-adotar").click();
  await page.getByTestId("suggest-acao-ok").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))].sort(), ["read_nodusctl", "run_action"]);
  await close();
});
