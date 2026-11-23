// A aba Compare num navegador, com o backend de mentira (ADR 0026). Prova que a aba pede só `compare_all`, só depois de aberta
// (e nunca manda caminho nem rótulo: a CLI compara todos os pares), que mostra um bloco por par com o que só existe de cada lado
// (pacman e Flatpak, dobrados em <details>) e os dotfiles que diferem com o estado de cada lado, que "não comparável" não é
// diferença, o aviso de SO diferente, o snapshot ilegível, o estado vazio, o erro do backend e que dado hostil vira texto.
// Dados inventados.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { launchBrowser, openApp, serveDist } from "./harness.mjs";

const inventoryText = readFileSync(new URL("../tests/fixtures/inventory.json", import.meta.url), "utf8");
const compareText = readFileSync(new URL("../tests/fixtures/compare_all.json", import.meta.url), "utf8");
const compareFixture = () => JSON.parse(compareText);

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

const handlers = (compare = compareText) => ({
  read_nodusctl: ({ command }) =>
    command === "inventory" ? inventoryText : command === "compare_all" ? compare : { __error: { kind: "not_allowed", message: command } },
});
const comandos = (calls) => calls.filter((c) => c.cmd === "read_nodusctl").map((c) => c.args.command);

async function abrirAba(page) {
  await page.getByTestId("system").waitFor();
  await page.getByTestId("tab-compare").click();
}

test("a aba Compare só pede `compare_all` depois de aberta, sem mandar caminho nem rótulo", async () => {
  const { page, calls, errors, close } = await openApp(browser, app, { handlers: handlers() });
  await page.getByTestId("system").waitFor();
  assert.deepEqual(comandos(calls), ["inventory"]);
  await page.getByTestId("tab-compare").click();
  await page.getByTestId("compare-par").first().waitFor();
  assert.deepEqual(comandos(calls), ["inventory", "compare_all"]);
  for (const c of calls) assert.deepEqual(Object.keys(c.args), ["command"], "só o nome do comando: a CLI compara todos os pares");
  assert.deepEqual(errors, []);
  await close();
});

test("um bloco por par, com os dois rótulos na ordem do JSON", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("compare-par").first().waitFor();
  assert.equal(await page.getByTestId("compare-par").count(), 2);
  assert.match(await page.getByTestId("compare-par").nth(0).innerText(), /pc-a × pc-b/);
  assert.match(await page.getByTestId("compare-par").nth(1).innerText(), /pc-a × pc-c/);
  await close();
});

test("pacman e Flatpak: a contagem em comum fica à vista e o que só existe de cada lado fica dobrado", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const par = page.getByTestId("compare-par").nth(0);
  await par.waitFor();
  assert.match(await par.getByTestId("compare-pacman").innerText(), /Pacman: 2 em comum/);
  assert.match(await par.getByTestId("compare-flatpak").innerText(), /Flatpak: 1 em comum/);
  const so = par.getByTestId("compare-so-em");
  // pacman: 1 só em pc-a e 2 só em pc-b; Flatpak: 1 só em pc-b (a lista vazia de pc-a não ganha um <details>).
  assert.equal(await so.count(), 3);
  const textos = [];
  for (let i = 0; i < 3; i++) textos.push(await so.nth(i).locator("summary").innerText());
  assert.deepEqual(textos, ["só em pc-a (1)", "só em pc-b (2)", "só em pc-b (1)"]);
  await close();
});

test("os <details> começam fechados e abrem com o clique, mostrando os itens", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const par = page.getByTestId("compare-par").nth(0);
  await par.waitFor();
  const primeiro = par.getByTestId("compare-so-em").first();
  assert.equal(await primeiro.evaluate((el) => el.open), false, "dobrado por padrão: a lista pode ter 150 itens");
  await primeiro.locator("summary").click();
  assert.equal(await primeiro.evaluate((el) => el.open), true);
  assert.match(await primeiro.innerText(), /vim/);
  await close();
});

test("dotfiles que diferem: o caminho e o estado de cada lado, em português, com 'fora da lista' para o caminho que a máquina não monitora", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const par = page.getByTestId("compare-par").nth(0);
  await par.waitFor();
  assert.match(await par.getByTestId("compare-dotfiles").innerText(), /Dotfiles: 7 em comum/);
  const linhas = par.getByTestId("compare-dotfile-difere");
  assert.equal(await linhas.count(), 2);
  assert.equal(await linhas.nth(0).innerText(), ".bashrc: pc-a ligado, pc-b arquivo");
  assert.equal(await linhas.nth(1).innerText(), ".zshrc: pc-a arquivo, pc-b fora da lista");
  await close();
});

test("os quatro estados de dotfile têm um rótulo cada", async () => {
  const data = compareFixture();
  data.comparisons[0].dotfiles.differ = [
    { path: "a", a: "linked", b: "missing" },
    { path: "b", a: "file", b: "other" },
    { path: "c", a: null, b: "linked" },
  ];
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  const linhas = page.getByTestId("compare-par").nth(0).getByTestId("compare-dotfile-difere");
  await linhas.first().waitFor();
  assert.equal(await linhas.nth(0).innerText(), "a: pc-a ligado, pc-b ausente");
  assert.equal(await linhas.nth(1).innerText(), "b: pc-a arquivo, pc-b outro");
  assert.equal(await linhas.nth(2).innerText(), "c: pc-a fora da lista, pc-b ligado");
  await close();
});

test("lista não comparável não é diferença: diz quem não tem e não acusa falta", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const par = page.getByTestId("compare-par").nth(1);
  await par.waitFor();
  const avisos = par.getByTestId("compare-nao-comparavel");
  assert.equal(await avisos.count(), 2);
  assert.match(await avisos.nth(0).innerText(), /Flatpak: não comparável, pc-c não tem Flatpak/);
  assert.match(await avisos.nth(1).innerText(), /Dotfiles: não comparável, pc-c não registrou dotfiles \(snapshot v1; rode `nodusctl snapshot` de novo\)/);
  assert.equal(await par.getByTestId("compare-dotfile-difere").count(), 0);
  assert.equal(await par.getByTestId("compare-so-em").count(), 0);
  await close();
});

test("com mais de uma máquina sem a capacidade, cita todas", async () => {
  const data = compareFixture();
  data.comparisons[1].flatpak.lacks = ["pc-a", "pc-c"];
  data.comparisons[1].dotfiles.lacks = ["pc-a", "pc-c"];
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  const avisos = page.getByTestId("compare-par").nth(1).getByTestId("compare-nao-comparavel");
  await avisos.first().waitFor();
  assert.match(await avisos.nth(0).innerText(), /pc-a e pc-c não têm Flatpak/);
  assert.match(await avisos.nth(1).innerText(), /pc-a e pc-c não registraram dotfiles/);
  await close();
});

test("SO diferente vira aviso e as listas continuam comparadas", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  const par = page.getByTestId("compare-par").nth(1);
  await par.waitFor();
  assert.match(await par.getByTestId("compare-so-diferente").innerText(), /sistemas diferentes: pc-a é Arch Linux, pc-c é Fedora Linux 41/);
  assert.match(await par.getByTestId("compare-pacman").innerText(), /Pacman: 3 em comum/);
  assert.equal(await page.getByTestId("compare-par").nth(0).getByTestId("compare-so-diferente").count(), 0);
  await close();
});

test("par sem nenhuma divergência diz 'sem divergências'; com divergência, não", async () => {
  const data = compareFixture();
  data.comparisons = [
    {
      a: "pc-a",
      b: "pc-b",
      os: null,
      pacman: { status: "compared", only_a: [], only_b: [], common: 3 },
      flatpak: { status: "compared", only_a: [], only_b: [], common: 0 },
      dotfiles: { status: "compared", differ: [], same: 9 },
    },
    data.comparisons[0],
  ];
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("compare-par").first().waitFor();
  assert.match(await page.getByTestId("compare-par").nth(0).getByTestId("compare-sem-divergencias").innerText(), /sem divergências/i);
  assert.equal(await page.getByTestId("compare-par").nth(1).getByTestId("compare-sem-divergencias").count(), 0);
  await close();
});

test("par só com listas não comparáveis diz 'sem divergências nas listas comparadas', nunca 'sem divergências' seco", async () => {
  const data = compareFixture();
  data.comparisons = [{ ...data.comparisons[1], os: null }];
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  const texto = await page.getByTestId("compare-sem-divergencias").innerText();
  assert.match(texto, /sem divergências nas listas comparadas/i);
  await close();
});

test("snapshot ilegível vira aviso com o rótulo e o motivo, sem esconder os pares", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("compare-ilegivel").waitFor();
  const texto = await page.getByTestId("compare-ilegivel").innerText();
  assert.match(texto, /quebrado/);
  assert.match(texto, /JSON inválido/);
  assert.equal(await page.getByTestId("compare-par").count(), 2);
  await close();
});

test("sem nenhum par: diz que são precisos dois snapshots e como gerá-los, sem blocos", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify({ comparisons: [], unreadable: [] })) });
  await abrirAba(page);
  await page.getByTestId("compare-vazio").waitFor();
  const texto = await page.getByTestId("compare-vazio").innerText();
  assert.match(texto, /pelo menos dois snapshots/i);
  assert.match(texto, /nodusctl snapshot/);
  assert.equal(await page.getByTestId("compare-par").count(), 0);
  await close();
});

test("sem par e com snapshot ilegível: o estado vazio e o aviso aparecem juntos", async () => {
  const data = { comparisons: [], unreadable: [{ label: "x", reason: "ilegível" }] };
  const { page, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("compare-vazio").waitFor();
  await page.getByTestId("compare-ilegivel").waitFor();
  await close();
});

test("com pares, não mostra o estado vazio", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("compare-par").first().waitFor();
  assert.equal(await page.getByTestId("compare-vazio").count(), 0);
  await close();
});

test("a nota diz que é só leitura e que o snapshot guarda `pacman -Qe`", async () => {
  const { page, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("compare-nota").waitFor();
  const texto = await page.getByTestId("compare-nota").innerText();
  assert.match(texto, /só leitura/i);
  assert.match(texto, /pacman -Qe/);
  await close();
});

test("erro do backend aparece como erro, e as outras abas continuam", async () => {
  const h = {
    read_nodusctl: ({ command }) =>
      command === "inventory" ? inventoryText : { __error: { kind: "failed", message: "falta a pasta-fonte: acrescente dotfiles_dir" } },
  };
  const { page, errors, close } = await openApp(browser, app, { handlers: h });
  await abrirAba(page);
  await page.getByTestId("erro").waitFor();
  assert.match(await page.getByTestId("erro").innerText(), /dotfiles_dir/);
  await page.getByTestId("tab-inventory").click();
  await page.getByTestId("system").waitFor();
  assert.deepEqual(errors, []);
  await close();
});

test("nomes hostis (rótulo, pacote, caminho, SO, motivo) viram texto: nada é executado nem vira elemento", async () => {
  const payload = '<img src=x onerror="window.__pwned=1">';
  const data = compareFixture();
  data.comparisons[0].a = payload;
  data.comparisons[0].pacman.only_a = [payload];
  data.comparisons[0].dotfiles.differ[0].path = payload;
  data.comparisons[1].os.b.name = payload;
  data.comparisons[1].flatpak.lacks = [payload];
  data.unreadable = [{ label: payload, reason: payload }];
  const { page, errors, close } = await openApp(browser, app, { handlers: handlers(JSON.stringify(data)) });
  await abrirAba(page);
  await page.getByTestId("compare-par").first().waitFor();
  await page.waitForTimeout(200);
  assert.equal(await page.evaluate(() => window.__pwned), undefined);
  assert.equal(await page.locator("img").count(), 0);
  assert.match(await page.getByTestId("compare-par").first().innerText(), /<img src=x/);
  assert.deepEqual(errors, []);
  await close();
});

test("a aba só pede read_nodusctl: nada que escreva", async () => {
  const { page, calls, close } = await openApp(browser, app, { handlers: handlers() });
  await abrirAba(page);
  await page.getByTestId("compare-par").first().waitFor();
  assert.deepEqual([...new Set(calls.map((c) => c.cmd))], ["read_nodusctl"]);
  await close();
});
