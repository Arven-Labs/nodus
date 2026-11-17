// O contrato do `nodusctl inventory --json` visto pela GUI (ADR 0026): parsing estrito dos campos conhecidos,
// erro claro para o que falta ou tem o tipo errado, e nada de confiar na forma do que veio do processo.
// A fixture é inventada (nenhum dado de máquina real). Formato: `crates/nodus-core/src/inventory.rs`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { parseInventory } from "../src/lib/inventory.ts";

const fixture = () => JSON.parse(readFileSync(new URL("./fixtures/inventory.json", import.meta.url), "utf8"));
const parse = (value) => parseInventory(typeof value === "string" ? value : JSON.stringify(value));
const rejects = (value) => assert.throws(() => parse(value), { name: "InventoryError" });

test("inventário válido: sistema, hardware, pacotes e apps Flatpak", () => {
  const inv = parse(fixture());
  assert.equal(inv.system.osName, "Arch Linux");
  assert.equal(inv.system.kernel, "9.9.9-test");
  assert.equal(inv.hardware.cpuModel, "Fictional CPU 9000");
  assert.equal(inv.hardware.cpuThreads, 8);
  assert.equal(inv.hardware.memoryTotalBytes, 17179869184);
  assert.equal(inv.hardware.disks.length, 1);
  assert.deepEqual(inv.packages.map((p) => p.name), ["alpha", "beta"]);
  assert.equal(inv.flatpakApps?.length, 1);
  assert.equal(inv.flatpakApps?.[0].application, "org.example.App");
  assert.equal(inv.flatpakApps?.[0].version, null);
});

test("flatpak_apps nulo significa Flatpak não instalado (e não lista vazia)", () => {
  const data = fixture();
  data.flatpak_apps = null;
  assert.equal(parse(data).flatpakApps, null);
  data.flatpak_apps = [];
  assert.deepEqual(parse(data).flatpakApps, []);
});

test("os_version nulo é aceito", () => {
  const data = fixture();
  data.system.os_version = null;
  assert.equal(parse(data).system.osVersion, null);
});

test("campos desconhecidos são ignorados (a CLI pode ganhar campos)", () => {
  const data = fixture();
  data.novo_campo = { qualquer: "coisa" };
  data.system.outro = 1;
  assert.equal(parse(data).system.osName, "Arch Linux");
});

test("falta de campo obrigatório é erro, com o nome do campo", () => {
  for (const campo of ["system", "hardware", "packages"]) {
    const data = fixture();
    delete data[campo];
    assert.throws(() => parse(data), (e) => e.name === "InventoryError" && e.message.includes(campo));
  }
  const data = fixture();
  delete data.system.kernel;
  assert.throws(() => parse(data), (e) => e.name === "InventoryError" && e.message.includes("kernel"));
});

test("tipo errado é erro", () => {
  const casos = [
    (d) => (d.packages = "alpha"),
    (d) => (d.packages = [{ name: 1, version: "1" }]),
    (d) => (d.packages = [{ name: "a" }]),
    (d) => (d.hardware.cpu_threads = "8"),
    (d) => (d.hardware.cpu_threads = 8.5),
    (d) => (d.hardware.memory_total_bytes = -1),
    (d) => (d.hardware.disks = {}),
    (d) => (d.system.os_name = null),
    (d) => (d.flatpak_apps = {}),
    (d) => (d.flatpak_apps = [{ application: "x" }]),
  ];
  for (const quebra of casos) {
    const data = fixture();
    quebra(data);
    rejects(data);
  }
});

test("texto que não é JSON, ou JSON que não é objeto, é erro", () => {
  rejects("not json");
  rejects("");
  rejects("[]");
  rejects("null");
  rejects("42");
});

test("inventário grande (20 mil pacotes) é aceito", () => {
  const data = fixture();
  data.packages = Array.from({ length: 20000 }, (_, i) => ({ name: `pkg-${i}`, version: "1-1" }));
  assert.equal(parse(data).packages.length, 20000);
});
