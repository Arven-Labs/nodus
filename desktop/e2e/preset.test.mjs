// A aba Preset num navegador, com o backend de mentira (ADR 0026). Prova que a aba pede só `preset_plan`, só depois de aberta
// (e nunca manda caminho: o arquivo vem de NODUS_PRESET, no Rust), que mostra o que falta, o que já está e o que o pacman não
// acha, o caso "nada a fazer", o Flatpak ausente, o erro do backend e que dado hostil vira texto. Dados inventados.

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

const handlers = (plan = planText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "preset_plan" ? plan : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-preset").click();
}

test("a aba Preset só pede `preset_plan` depois de aberta, sem mandar caminho", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  await page.getByTestId("tab-preset").click();
  await page.getByTestId("preset-pacman").waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "preset_plan"]);
  for (const c of calls) assert.deepEqual(Object.keys(c.args), ["command"], "só o nome do comando: o caminho vem do Rust");
  assert.deepEqual(errors, []);
  await close();
});

test("mostra o que falta instalar e quantos já estão presentes, pacman e Flatpak", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-pacman").waitFor();
  const pacman = await page.getByTestId("preset-pacman").innerText();
  assert.match(pacman, /instalar 2/);
  assert.match(pacman, /150 já presentes/);
  assert.equal(await page.getByTestId("preset-pacman-item").count(), 2);
  assert.match(pacman, /htop/);
  assert.match(pacman, /ripgrep/);
  const flatpak = await page.getByTestId("preset-flatpak").innerText();
  assert.match(flatpak, /instalar 1/);
  assert.match(flatpak, /4 já presentes/);
  assert.match(flatpak, /org\.gnome\.Calculator/);
  await close();
});

test("os indisponíveis ficam numa seção própria que diz que o nodus não instala", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-indisponiveis").waitFor();
  const texto = await page.getByTestId("preset-indisponiveis").innerText();
  assert.match(texto, /brave-bin/);
  assert.match(texto, /não instala/i);
  // e não se misturam com os que serão instalados
  assert.doesNotMatch(await page.getByTestId("preset-pacman").innerText(), /brave-bin/);
  await close();
});

test("sem indisponíveis, a seção não aparece", async () => {
  const data = planFixture();
  data.pacman_unavailable = [];
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("preset-pacman").waitFor();
  assert.equal(await page.getByTestId("preset-indisponiveis").count(), 0);
  await close();
});

test("nada a fazer: a máquina já está como o preset", async () => {
  const data = { ...planFixture(), pacman_missing: [], pacman_unavailable: [], flatpak_missing: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("preset-nada").waitFor();
  assert.match(await page.getByTestId("preset-nada").innerText(), /nada a fazer/i);
  await close();
});

test("só restam indisponíveis: diz que não há o que instalar, não 'nada a fazer'", async () => {
  const data = { ...planFixture(), pacman_missing: [], flatpak_missing: [] };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("preset-nada").waitFor();
  const texto = await page.getByTestId("preset-nada").innerText();
  assert.match(texto, /nada a instalar/i);
  assert.match(texto, /indisponíveis/i);
  assert.doesNotMatch(texto, /nada a fazer/i);
  await close();
});

test("com itens a instalar, a mensagem de 'nada' não aparece", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-pacman").waitFor();
  assert.equal(await page.getByTestId("preset-nada").count(), 0);
  await close();
});

test("Flatpak não instalado: avisa, e os apps do preset aparecem como pendentes", async () => {
  const data = { ...planFixture(), flatpak_installed: false, flatpak_present: 0 };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("preset-flatpak").waitFor();
  const texto = await page.getByTestId("preset-flatpak").innerText();
  assert.match(texto, /flatpak não instalado/i);
  assert.match(texto, /org\.gnome\.Calculator/);
  assert.doesNotMatch(texto, /já presentes/);
  await close();
});

test("a aba diz que é só leitura e que instalar é com o terminal", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-nota").waitFor();
  const texto = await page.getByTestId("preset-nota").innerText();
  assert.match(texto, /nada foi executado/i);
  assert.match(texto, /preset apply/);
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
  const hostil = { ...planFixture(), pacman_missing: [payload], pacman_unavailable: [payload], flatpak_missing: [payload] };
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(hostil)) });
  await abrirAba(page);
  await page.getByTestId("preset-pacman").waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("preset-pacman").innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("a aba só pede read_nodusctl: nada que escreva", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("preset-pacman").waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  await close();
});
