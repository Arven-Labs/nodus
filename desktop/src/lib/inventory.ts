// O contrato do `nodusctl inventory --json` visto pela GUI (ADR 0026). Formato: `crates/nodus-core/src/inventory.rs`.
//
// A saída do processo é dado externo: aqui o parsing é estrito nos campos conhecidos (falta ou tipo errado é erro, com o caminho
// do campo), e campos desconhecidos são ignorados (a CLI pode ganhar campos). Nada de `any`, nada de confiar na forma.
// Só sintaxe que o Node entende ao remover os tipos (sem enums, sem parâmetros de propriedade): os testes importam este arquivo.

import { array, count, field, nullableText, object, ParseError, text } from "./parse.ts";

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

export class InventoryError extends ParseError {
  name = "InventoryError";
}

/** Os helpers de `parse.ts` lançam `ParseError`; aqui o erro vira `InventoryError` para a tela saber de onde veio. */
export function parseInventory(jsonText: string): Inventory {
  try {
    return parseInventoryFields(jsonText);
  } catch (error) {
    if (error instanceof ParseError && !(error instanceof InventoryError)) throw new InventoryError(error.message);
    throw error;
  }
}

function parseInventoryFields(jsonText: string): Inventory {
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
