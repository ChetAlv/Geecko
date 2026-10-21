import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, isAbsolute, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createStore } from './database.js';
import { ValidationError, validateAsset, validateWorkOrder } from './domain.js';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const PUBLIC_DIR = join(ROOT, 'public');
const DATABASE_PATH = process.env.GEECKO_DB_PATH || join(ROOT, 'data', 'geecko.db');
const HOST = '127.0.0.1';
const PORT = Number(process.env.PORT || 4177);
const MAX_BODY_BYTES = 1_000_000;
const store = createStore(DATABASE_PATH);

const MIME_TYPES = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon'
};

function securityHeaders(contentType = 'application/json; charset=utf-8') {
  return {
    'Content-Type': contentType,
    'Content-Security-Policy': "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self'; font-src 'self'; object-src 'none'; base-uri 'none'; form-action 'self'; frame-ancestors 'none'",
    'X-Content-Type-Options': 'nosniff',
    'Referrer-Policy': 'no-referrer',
    'Cache-Control': 'no-store'
  };
}

function sendJson(response, status, payload, extraHeaders = {}) {
  response.writeHead(status, { ...securityHeaders(), ...extraHeaders });
  response.end(JSON.stringify(payload));
}

async function readJson(request) {
  let bytes = 0;
  const chunks = [];
  for await (const chunk of request) {
    bytes += chunk.length;
    if (bytes > MAX_BODY_BYTES) {
      const error = new Error('El contenido enviado es demasiado grande.');
      error.status = 413;
      throw error;
    }
    chunks.push(chunk);
  }
  if (!chunks.length) return {};
  try {
    return JSON.parse(Buffer.concat(chunks).toString('utf8'));
  } catch {
    const error = new Error('El contenido JSON no es válido.');
    error.status = 400;
    throw error;
  }
}

function routeMatch(pathname, pattern) {
  const match = pathname.match(pattern);
  return match ? match.slice(1).map(Number) : null;
}

async function handleApi(request, response, url) {
  if (request.method === 'GET' && url.pathname === '/api/health') {
    return sendJson(response, 200, { ok: true, mode: 'offline', database: 'sqlite' });
  }

  if (request.method === 'GET' && url.pathname === '/api/dashboard') {
    return sendJson(response, 200, store.getDashboard());
  }

  if (request.method === 'GET' && url.pathname === '/api/orders') {
    return sendJson(response, 200, {
      orders: store.listOrders({
        status: url.searchParams.get('status') || '',
        priority: url.searchParams.get('priority') || '',
        search: (url.searchParams.get('search') || '').trim().slice(0, 100)
      })
    });
  }

  const orderRoute = routeMatch(url.pathname, /^\/api\/orders\/(\d+)$/);
  if (request.method === 'GET' && orderRoute) {
    const order = store.getOrder(orderRoute[0]);
    return order
      ? sendJson(response, 200, order)
      : sendJson(response, 404, { error: 'No encontramos esa orden.' });
  }

  if (request.method === 'POST' && url.pathname === '/api/orders') {
    const data = validateWorkOrder(await readJson(request));
    return sendJson(response, 201, store.createOrder(data));
  }

  if (request.method === 'PATCH' && orderRoute) {
    const data = validateWorkOrder(await readJson(request), { partial: true });
    const order = store.updateOrder(orderRoute[0], data);
    return order
      ? sendJson(response, 200, order)
      : sendJson(response, 404, { error: 'No encontramos esa orden.' });
  }

  const taskRoute = routeMatch(url.pathname, /^\/api\/orders\/(\d+)\/tasks\/(\d+)$/);
  if (request.method === 'PATCH' && taskRoute) {
    const body = await readJson(request);
    const order = store.setTaskCompleted(taskRoute[0], taskRoute[1], Boolean(body.completed));
    return order
      ? sendJson(response, 200, order)
      : sendJson(response, 404, { error: 'No encontramos esa tarea.' });
  }

  if (request.method === 'GET' && url.pathname === '/api/assets') {
    return sendJson(response, 200, { assets: store.listAssets() });
  }

  if (request.method === 'POST' && url.pathname === '/api/assets') {
    const data = validateAsset(await readJson(request));
    return sendJson(response, 201, store.createAsset(data));
  }

  if (request.method === 'GET' && url.pathname === '/api/export') {
    const filename = `geecko-respaldo-${new Date().toISOString().slice(0, 10)}.json`;
    return sendJson(response, 200, store.exportAll(), {
      'Content-Disposition': `attachment; filename="${filename}"`
    });
  }

  return sendJson(response, 404, { error: 'Ruta no encontrada.' });
}

async function serveStatic(response, pathname, headOnly = false) {
  const requested = pathname === '/' ? 'index.html' : decodeURIComponent(pathname.replace(/^\/+/, ''));
  const filePath = resolve(PUBLIC_DIR, requested);
  const relativePath = relative(PUBLIC_DIR, filePath);
  if (relativePath.startsWith('..') || isAbsolute(relativePath)) {
    return sendJson(response, 403, { error: 'Acceso denegado.' });
  }

  try {
    const body = await readFile(filePath);
    response.writeHead(200, securityHeaders(MIME_TYPES[extname(filePath)] || 'application/octet-stream'));
    response.end(headOnly ? undefined : body);
  } catch (error) {
    if (error.code === 'ENOENT') return sendJson(response, 404, { error: 'Archivo no encontrado.' });
    throw error;
  }
}

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, `http://${HOST}:${PORT}`);
    if (url.pathname.startsWith('/api/')) {
      await handleApi(request, response, url);
    } else if (request.method === 'GET' || request.method === 'HEAD') {
      await serveStatic(response, url.pathname, request.method === 'HEAD');
    } else {
      sendJson(response, 405, { error: 'Método no permitido.' }, { Allow: 'GET, HEAD' });
    }
  } catch (error) {
    if (error instanceof ValidationError) {
      return sendJson(response, 422, { error: error.message, fields: error.fields });
    }
    if (error.code === 'DUPLICATE_ASSET_CODE') {
      return sendJson(response, 409, { error: error.message, fields: { code: error.message } });
    }
    if (String(error.message).includes('FOREIGN KEY constraint failed')) {
      return sendJson(response, 422, { error: 'El equipo seleccionado ya no existe.', fields: { assetId: 'Selecciona otro equipo.' } });
    }
    const status = error.status || 500;
    if (status >= 500) console.error(error);
    sendJson(response, status, { error: status >= 500 ? 'Ocurrió un error interno.' : error.message });
  }
});

function shutdown() {
  server.close(() => {
    store.close();
    process.exit(0);
  });
}

process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);

server.listen(PORT, HOST, () => {
  console.log(`Geecko Maintenance listo en http://${HOST}:${PORT}`);
  console.log(`Datos locales: ${DATABASE_PATH}`);
});
