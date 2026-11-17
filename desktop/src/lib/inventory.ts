// O contrato do `nodusctl inventory --json` visto pela GUI (ADR 0026). Formato: `crates/nodus-core/src/inventory.rs`.
//
// A saída do processo é dado externo: aqui o parsing é estrito nos campos conhecidos (falta ou tipo errado é erro, com o caminho
// do campo), e campos desconhecidos são ignorados (a CLI pode ganhar campos). Nada de `any`, nada de confiar na forma.
// Só sintaxe que o Node entende ao remover os tipos (sem enums, sem parâmetros de propriedade): os testes importam este arquivo.

export interface Disk {
  name: string;
  mountPoint: string;
  fileSystem: string;
  totalBytes: number;
}

export interface FlatpakApp {
  application: string;
  version: string | null;
  branch: string;
  origin: string;
  installation: string;
}

export interface Pkg {
  name: string;
  version: string;
}

export interface Inventory {
  system: { osName: string; osVersion: string | null; kernel: string };
  hardware: { cpuModel: string; cpuThreads: number; memoryTotalBytes: number; disks: Disk[] };
  packages: Pkg[];
  /** `null`: Flatpak não instalado. `[]`: instalado, sem apps. */
  flatpakApps: FlatpakApp[] | null;
}

export class InventoryError extends Error {
  name = "InventoryError";
}

function fail(path: string, expected: string): never {
  throw new InventoryError(`campo \`${path}\`: esperava ${expected}`);
}

function object(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail(path, "um objeto");
  return value as Record<string, unknown>;
}

function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) fail(path, "uma lista");
  return value;
}

function text(value: unknown, path: string): string {
  if (typeof value !== "string") fail(path, "texto");
  return value;
}

function nullableText(value: unknown, path: string): string | null {
  if (value === null) return null;
  return text(value, path);
}

function count(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) fail(path, "um inteiro não negativo");
  return value;
}

function field(parent: Record<string, unknown>, key: string, path: string): unknown {
  if (!(key in parent)) fail(`${path}${key}`, "o campo (ausente)");
  return parent[key];
}

export function parseInventory(jsonText: string): Inventory {
  let raw: unknown;
  try {
    raw = JSON.parse(jsonText);
  } catch {
    throw new InventoryError("a saída do nodusctl não é JSON");
  }
  const root = object(raw, "(raiz)");

  const system = object(field(root, "system", ""), "system");
  const hardware = object(field(root, "hardware", ""), "hardware");
  const packages = array(field(root, "packages", ""), "packages");
  const flatpak = field(root, "flatpak_apps", "");

  return {
    system: {
      osName: text(field(system, "os_name", "system."), "system.os_name"),
      osVersion: nullableText(field(system, "os_version", "system."), "system.os_version"),
      kernel: text(field(system, "kernel", "system."), "system.kernel"),
    },
    hardware: {
      cpuModel: text(field(hardware, "cpu_model", "hardware."), "hardware.cpu_model"),
      cpuThreads: count(field(hardware, "cpu_threads", "hardware."), "hardware.cpu_threads"),
      memoryTotalBytes: count(field(hardware, "memory_total_bytes", "hardware."), "hardware.memory_total_bytes"),
      disks: array(field(hardware, "disks", "hardware."), "hardware.disks").map((item, i) => {
        const d = object(item, `hardware.disks[${i}]`);
        return {
          name: text(field(d, "name", `hardware.disks[${i}].`), `hardware.disks[${i}].name`),
          mountPoint: text(field(d, "mount_point", `hardware.disks[${i}].`), `hardware.disks[${i}].mount_point`),
          fileSystem: text(field(d, "file_system", `hardware.disks[${i}].`), `hardware.disks[${i}].file_system`),
          totalBytes: count(field(d, "total_bytes", `hardware.disks[${i}].`), `hardware.disks[${i}].total_bytes`),
        };
      }),
    },
    packages: packages.map((item, i) => {
      const p = object(item, `packages[${i}]`);
      return {
        name: text(field(p, "name", `packages[${i}].`), `packages[${i}].name`),
        version: text(field(p, "version", `packages[${i}].`), `packages[${i}].version`),
      };
    }),
    flatpakApps:
      flatpak === null
        ? null
        : array(flatpak, "flatpak_apps").map((item, i) => {
            const a = object(item, `flatpak_apps[${i}]`);
            const at = `flatpak_apps[${i}].`;
            return {
              application: text(field(a, "application", at), `${at}application`),
              version: nullableText(field(a, "version", at), `${at}version`),
              branch: text(field(a, "branch", at), `${at}branch`),
              origin: text(field(a, "origin", at), `${at}origin`),
              installation: text(field(a, "installation", at), `${at}installation`),
            };
          }),
  };
}
