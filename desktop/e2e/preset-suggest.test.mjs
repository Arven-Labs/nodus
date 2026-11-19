// A aba Sugestões num navegador, com o backend de mentira (ADR 0026). Prova que a aba pede só `preset_suggest`, só depois de
// aberta (e nunca manda caminho: o arquivo vem de NODUS_PRESET, no Rust), que separa o que o `apply` instala do que é de fora
// dos repositórios, conta os escondidos pelo config, mostra o caso "nada fora do preset", o Flatpak ausente, o erro do backend
// e que dado hostil vira texto. Dados inventados.

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

const handlers = (suggest = suggestText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "preset_suggest" ? suggest : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-suggest").click();
}

test("a aba Sugestões só pede `preset_suggest` depois de aberta, sem mandar caminho", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  await page.getByTestId("tab-suggest").click();
  await page.getByTestId("suggest-pacman").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "preset_suggest"]);
  for (const c of calls) assert.deepEqual(Object.keys(c.args), ["command"], "só o nome do comando: o caminho vem do Rust");
  assert.deepEqual(errors, []);
  await close();
});

test("lista o que está instalado de propósito e fora do preset, pacman e Flatpak", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-pacman").waitFor();
  const pacman = await page.getByTestId("suggest-pacman").innerText();
  assert.match(pacman, /Pacman \(2\)/);
  assert.equal(await page.getByTestId("suggest-pacman-item").count(), 2);
  assert.match(pacman, /neovim/);
  assert.match(pacman, /tmux/);
  const flatpak = await page.getByTestId("suggest-flatpak").innerText();
  assert.match(flatpak, /Flatpak \(1\)/);
  assert.match(flatpak, /org\.mozilla\.firefox/);
  await close();
});

test("os de fora dos repositórios ficam numa seção própria que diz que o apply não instala", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-foreign").waitFor();
  const texto = await page.getByTestId("suggest-foreign").innerText();
  assert.match(texto, /brave-bin/);
  assert.match(texto, /apply/);
  assert.match(texto, /não instala/i);
  // e não se misturam com os que o apply instalaria
  assert.doesNotMatch(await page.getByTestId("suggest-pacman").innerText(), /brave-bin/);
  await close();
});

test("sem itens de fora dos repositórios, a seção não aparece", async () => {
  const data = suggestFixture();
  data.foreign_extra = [];
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("suggest-pacman").waitFor();
  assert.equal(await page.getByTestId("suggest-foreign").count(), 0);
  await close();
});

test("conta os escondidos pelo config e diz como ver todos no terminal", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-ignorados").waitFor();
  const texto = await page.getByTestId("suggest-ignorados").innerText();
  assert.match(texto, /2 ignorados/);
  assert.match(texto, /--all/);
  // a lista não é mostrada como sugestão: só a contagem
  assert.doesNotMatch(await page.getByTestId("suggest-pacman").innerText(), /cmatrix|\bsl\b/);
  await close();
});

test("um escondido usa o singular, e sem escondidos o aviso não aparece", async () => {
  const um = { ...suggestFixture(), ignored: ["sl"] };
  const a = await openApp(browser, app, { handlers: handlers(JSON.stringify(um)) });
  await abrirAba(a.page);
  await a.page.getByTestId("suggest-ignorados").waitFor();
  assert.match(await a.page.getByTestId("suggest-ignorados").innerText(), /1 ignorado\b/);
  await a.close();

  const nenhum = { ...suggestFixture(), ignored: [] };
  const b = await openApp(browser, app, { handlers: handlers(JSON.stringify(nenhum)) });
  await abrirAba(b.page);
  await b.page.getByTestId("suggest-pacman").waitFor();
  assert.equal(await b.page.getByTestId("suggest-ignorados").count(), 0);
  await b.close();
});

test("nada fora do preset: tudo o que foi instalado de propósito já está nele", async () => {
  const data = { ...suggestFixture(), pacman_extra: [], foreign_extra: [], flatpak_extra: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("suggest-nada").waitFor();
  assert.match(await page.getByTestId("suggest-nada").innerText(), /nada fora do preset/i);
  await close();
});

test("com sugestões, a mensagem de 'nada' não aparece", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-pacman").waitFor();
  assert.equal(await page.getByTestId("suggest-nada").count(), 0);
  await close();
});

test("só restam os de fora dos repositórios: não é 'nada fora do preset'", async () => {
  const data = { ...suggestFixture(), pacman_extra: [], flatpak_extra: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("suggest-foreign").waitFor();
  assert.equal(await page.getByTestId("suggest-nada").count(), 0);
  await close();
});

test("Flatpak não instalado: avisa que os apps não foram conferidos", async () => {
  const data = { ...suggestFixture(), flatpak_installed: false, flatpak_extra: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("suggest-flatpak").waitFor();
  const texto = await page.getByTestId("suggest-flatpak").innerText();
  assert.match(texto, /flatpak não instalado/i);
  assert.match(texto, /não foram conferidos/i);
  await close();
});

test("a aba diz que é só leitura e que adotar é com o terminal", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-nota").waitFor();
  const texto = await page.getByTestId("suggest-nota").innerText();
  assert.match(texto, /nada foi alterado/i);
  assert.match(texto, /preset adopt/);
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

test("nomes hostis viram texto: nada é executado nem vira elemento", async () => {
  const payload = '<img src=x onerror="window.__pwned=1">';
  const hostil = { ...suggestFixture(), pacman_extra: [payload], foreign_extra: [payload], flatpak_extra: [payload] };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(hostil)) });
  await abrirAba(page);
  await page.getByTestId("suggest-pacman").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("suggest-pacman").innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("a aba só pede read_nodusctl: nada que escreva", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("suggest-pacman").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  await close();
});
