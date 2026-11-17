// O que os testes e2e da tela precisam: a página compilada (`desktop/dist`) servida de uma pasta, um navegador sem janela e um
// backend Tauri de mentira. Não há `tauri-driver`: o `invoke` da página é respondido pelo teste com dados inventados (o que
// `@tauri-apps/api/mocks` faz), então o que se prova é a tela e o que ela pede, não o Rust por trás (esse é do `cargo test`).
// Modelo: `warden/desktop/e2e/harness.mjs`.
//
// Precisa, uma vez: `npm run build` (gera `dist`) e um Chrome ou Chromium (`npx playwright-core install chromium`).

import fs from "node:fs";
import http from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const distDir = path.join(desktopRoot, "dist");

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".svg": "image/svg+xml",
  ".json": "application/json",
};

/** Serve `desktop/dist` numa porta livre; `stop()` fecha. */
export async function serveDist() {
  if (!fs.existsSync(path.join(distDir, "index.html"))) {
    throw new Error(`Não há build em ${distDir}. Rode \`npm run build\` em desktop/ antes.`);
  }
  const server = http.createServer((req, res) => {
    const url = new URL(req.url ?? "/", "http://localhost");
    const file = path.join(distDir, path.normalize(decodeURIComponent(url.pathname)));
    const inside = file.startsWith(distDir) && fs.existsSync(file) && fs.statSync(file).isFile();
    const served = inside ? file : path.join(distDir, "index.html");
    res.writeHead(200, { "content-type": TYPES[path.extname(served)] ?? "application/octet-stream" });
    fs.createReadStream(served).pipe(res);
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return {
    url: `http://127.0.0.1:${server.address().port}/`,
    stop: () => new Promise((resolve) => server.close(resolve)),
  };
}

export async function launchBrowser() {
  const executablePath = process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE;
  const attempts = executablePath ? [{ executablePath }] : [{}, { channel: "chrome" }];
  let last;
  for (const options of attempts) {
    try {
      return await chromium.launch(options);
    } catch (err) {
      last = err;
    }
  }
  throw new Error(
    `Não há navegador para dirigir. Instale com \`npx playwright-core install chromium\` ou aponte PLAYWRIGHT_CHROMIUM_EXECUTABLE. (${last})`,
  );
}

/** O backend de mentira dentro da página: a forma que `@tauri-apps/api` procura, com todo `invoke` entregue ao teste. */
const BACKEND = `(() => {
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { windowLabel: "main", label: "main" } },
    invoke: async (cmd, args) => {
      const result = await window.__nodus_invoke(cmd, args ?? {});
      if (result && result.__error) throw result.__error;
      return result;
    },
    transformCallback: () => 0,
    unregisterCallback: () => {},
    convertFileSrc: (p) => p,
  };
})();`;

/**
 * Abre a página com `handlers` respondendo o backend: nome do comando para um valor ou função dos argumentos. Para um erro como o
 * do Rust (um objeto `{ kind, message }`), o handler devolve `{ __error: { kind, message } }`. Comando sem handler devolve `null`.
 * Devolve a página, todas as chamadas (`calls`: `{ cmd, args }`) e os erros não tratados da página (`errors`), que o teste afirma vazios.
 */
export async function openApp(browser, app, { handlers = {} } = {}) {
  const context = await browser.newContext({ locale: "pt-BR", viewport: { width: 1100, height: 800 } });
  await context.addInitScript(BACKEND);
  const page = await context.newPage();
  const calls = [];
  const errors = [];
  page.on("pageerror", (err) => errors.push(err.message));
  await page.exposeFunction("__nodus_invoke", async (cmd, args) => {
    calls.push({ cmd, args });
    const handler = handlers[cmd];
    if (handler === undefined) return null;
    return typeof handler === "function" ? handler(args) : handler;
  });
  await page.goto(app.url);
  return { page, calls, errors, close: () => context.close() };
}
