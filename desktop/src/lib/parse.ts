// Helpers de parsing estrito compartilhados pelas telas (ADR 0026). A saída do `nodusctl` é dado externo: falta de campo ou tipo
// errado é erro, com o caminho do campo. Só sintaxe que o Node entende ao remover os tipos (os testes importam estes arquivos).

export class ParseError extends Error {
  name = "ParseError";
}

export function fail(path: string, expected: string): never {
  throw new ParseError(`campo \`${path}\`: esperava ${expected}`);
}

export function object(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) fail(path, "um objeto");
  return value as Record<string, unknown>;
}

export function array(value: unknown, path: string): unknown[] {
  if (!Array.isArray(value)) fail(path, "uma lista");
  return value;
}

export function text(value: unknown, path: string): string {
  if (typeof value !== "string") fail(path, "texto");
  return value;
}

export function nullableText(value: unknown, path: string): string | null {
  if (value === null) return null;
  return text(value, path);
}

export function count(value: unknown, path: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value)) fail(path, "um inteiro não negativo");
  return value;
}

export function nullableCount(value: unknown, path: string): number | null {
  if (value === null) return null;
  return count(value, path);
}

export function bool(value: unknown, path: string): boolean {
  if (typeof value !== "boolean") fail(path, "verdadeiro ou falso");
  return value;
}

export function field(parent: Record<string, unknown>, key: string, path: string): unknown {
  if (!(key in parent)) fail(`${path}${key}`, "o campo (ausente)");
  return parent[key];
}
