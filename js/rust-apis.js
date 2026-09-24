/* Only the API surface reached by the anonymous listing probe is installed. */
export function host(op, args) {
  const result = JSON.parse(
    globalThis.nativeTransport(JSON.stringify({ op, args })),
  );
  if (result.error) throw new Error(result.error);
  return result.value;
}

function TextEncoder() {
  this.encoding = "utf-8";
}
TextEncoder.prototype.encode = function (text = "") {
  const units = [];
  text = String(text);
  for (let i = 0; i < text.length; i++) units.push(text.charCodeAt(i));
  return new Uint8Array(host("encode", { units }));
};

function TextDecoder(label = "utf-8", options = {}) {
  if (!["utf-8", "utf8"].includes(label.toLowerCase()))
    throw new Error("Only UTF-8 is used by this probe");
  this.fatal = !!options.fatal;
  this.ignoreBOM = !!options.ignoreBOM;
  this.encoding = "utf-8";
}
TextDecoder.prototype.decode = function (
  buffer = new Uint8Array(),
  options = {},
) {
  if (options.stream)
    throw new Error("Streaming text decode is outside this probe");
  const bytes = ArrayBuffer.isView(buffer)
    ? new Uint8Array(buffer.buffer, buffer.byteOffset, buffer.byteLength)
    : new Uint8Array(buffer);
  return host("decode", {
    bytes: Array.from(bytes),
    fatal: this.fatal,
    ignoreBOM: this.ignoreBOM,
  });
};

function URLSearchParams(input = "") {
  this.pairs =
    typeof input === "string"
      ? host("query_parse", { query: input.replace(/^\?/, "") })
      : Array.isArray(input)
        ? input.map(([key, value]) => [String(key), String(value)])
        : Object.entries(input).map(([key, value]) => [key, String(value)]);
}
URLSearchParams.prototype.get = function (name) {
  return this.pairs.find(([key]) => key === String(name))?.[1] ?? null;
};
URLSearchParams.prototype.has = function (name) {
  return this.get(name) !== null;
};
URLSearchParams.prototype.set = function (name, value) {
  name = String(name);
  const index = this.pairs.findIndex(([key]) => key === name);
  this.pairs = this.pairs.filter(([key], i) => key !== name || i === index);
  if (index < 0) this.pairs.push([name, String(value)]);
  else this.pairs[index] = [name, String(value)];
  this.commit?.();
};
URLSearchParams.prototype.toString = function () {
  return host("query_string", { pairs: this.pairs });
};
URLSearchParams.prototype[Symbol.iterator] = function () {
  return this.pairs[Symbol.iterator]();
};

function URL(input, base) {
  this.parts = host("url", {
    input: String(input),
    base: base === undefined ? null : String(base),
  });
  this.searchParams = new URLSearchParams(this.parts.search);
  const owner = this;
  this.searchParams.commit = function () {
    owner.parts = host("url", {
      input: owner.parts.href,
      query: owner.searchParams.toString(),
    });
  };
}
for (const property of [
  "href",
  "origin",
  "host",
  "hostname",
  "pathname",
  "protocol",
  "search",
  "hash",
]) {
  Object.defineProperty(URL.prototype, property, {
    get() {
      return this.parts[property];
    },
  });
}
URL.prototype.toString = function () {
  return this.href;
};
URL.prototype.toJSON = function () {
  return this.href;
};

function Headers(init = []) {
  this.pairs = host("headers", {
    pairs:
      init instanceof Headers
        ? init.pairs
        : Array.isArray(init)
          ? init
          : Object.entries(init),
    action: "normalize",
  });
}
for (const action of ["get", "set", "append", "delete", "has"]) {
  Headers.prototype[action] = function (name, value) {
    const result = host("headers", {
      pairs: this.pairs,
      action,
      name: String(name),
      value: value === undefined ? null : String(value),
    });
    if (["set", "append", "delete"].includes(action)) this.pairs = result;
    else return result;
  };
}
Headers.prototype[Symbol.iterator] = function () {
  return this.pairs[Symbol.iterator]();
};

function Request(input, init = {}) {
  this.url = String(input instanceof Request ? input.url : input);
  this.method = init.method || input.method || "GET";
  this.headers = new Headers(init.headers || input.headers);
  this.body = init.body ?? input.body ?? null;
  this.redirect = init.redirect || input.redirect || "follow";
}

export async function fetch(input, init = {}) {
  const request = new Request(input, init);
  if (request.body !== null && typeof request.body !== "string")
    throw new Error("This probe only sends JSON request bodies");
  const result = host("fetch", { ...request, headers: [...request.headers] });
  let consumed = false;
  function consume() {
    if (consumed) throw new TypeError("Response body already consumed");
    consumed = true;
    return result.body;
  }
  return {
    status: result.status,
    url: result.url,
    ok: result.status >= 200 && result.status < 300,
    headers: new Headers(result.headers),
    get bodyUsed() {
      return consumed;
    },
    async text() {
      return consume();
    },
    async json() {
      return JSON.parse(consume());
    },
  };
}

const listeners = new WeakMap();
function EventTarget() {
  listeners.set(this, new Map());
}
EventTarget.prototype.addEventListener = function (name, listener) {
  const byName = listeners.get(this);
  if (!byName.has(name)) byName.set(name, new Set());
  byName.get(name).add(listener);
};
EventTarget.prototype.removeEventListener = function (name, listener) {
  listeners.get(this).get(name)?.delete(listener);
};
EventTarget.prototype.dispatchEvent = function (event) {
  const pending = Array.from(listeners.get(this).get(event.type) || []);
  for (const listener of pending) listener.call(this, event);
  return true;
};
export function CustomEvent(type, options = {}) {
  this.type = type;
  this.detail = options.detail;
}

globalThis.TextEncoder = TextEncoder;
globalThis.TextDecoder = TextDecoder;
globalThis.URL = URL;
globalThis.URLSearchParams = URLSearchParams;
globalThis.Headers = Headers;
globalThis.Request = Request;
globalThis.EventTarget = EventTarget;
