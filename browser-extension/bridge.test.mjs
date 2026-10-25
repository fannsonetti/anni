import test from "node:test";
import assert from "node:assert/strict";
import {createBridge, pairingFromUrl, innaCookie} from "./bridge.mjs";

const token = "a".repeat(64);
const url = `http://127.0.0.1:54321/connect#${token}`;
function fixture() {
  const store = {}, requests = [], reads = [];
  const api = {
    runtime: {id: "test-extension", getURL: name => `chrome-extension://test-extension/${name}`},
    storage: {session: {get: async key => ({[key]: store[key]}), set: async value => Object.assign(store, value), remove: async key => {delete store[key];}}},
    cookies: {getAll: async filter => {
      reads.push(filter);
      return [{name: "INNA_SESSION", value: "synthetic-inna-session", domain: ".inna.is", path: "/", httpOnly: true, secure: true},
        {name: "GOOGLE_SESSION", value: "must-never-transfer", domain: ".google.com", path: "/"}];
    }}
  };
  const request = async (target, options) => {
    requests.push({target, options});
    return target.endsWith("/info") ? {ok: true, json: async () => ({protocol: "anni-browser-connect-v1"})} : {status: 204};
  };
  const popup = {id: api.runtime.id, url: api.runtime.getURL("popup.html")};
  const page = {id: api.runtime.id, frameId: 0, url, tab: {url}};
  return {api, store, requests, reads, popup, page, request};
}

test("pairing allows only a temporary capability on literal loopback", () => {
  assert.equal(pairingFromUrl(url, 1000).expires, 601000);
  for (const value of [url.replace("127.0.0.1", "attacker.example"), url.replace("http:", "https:"), url.replace("/connect", "/other"), url.replace(`#${token}`, "#short"), url.replace("127.0.0.1", "127.0.0.1.attacker.example")]) {
    assert.equal(pairingFromUrl(value), null);
  }
});

test("only an explicit popup action reads and transfers the INNA session", async () => {
  const f = fixture(), handle = createBridge(f.api, f.request, () => 1000);
  await handle({type: "PAIR"}, f.page);
  assert.equal(f.reads.length, 0);
  assert.equal((await handle({type: "STATUS"}, f.popup)).paired, true);
  assert.equal(f.reads.length, 0);
  await assert.rejects(handle({type: "CONNECT"}, f.page), /popup/);
  assert.equal((await handle({type: "CONNECT"}, f.popup)).connected, true);
  assert.deepEqual(f.reads, [{url: "https://nam.inna.is/"}]);
  const sent = f.requests.at(-1);
  assert.equal(sent.target, "http://127.0.0.1:54321/session");
  assert.equal(sent.options.headers.Authorization, `Bearer ${token}`);
  assert.equal(sent.options.redirect, "error");
  assert.equal(sent.options.credentials, "omit");
  assert.ok(sent.options.body.includes("synthetic-inna-session"));
  assert.ok(!sent.options.body.includes("must-never-transfer"));
  assert.equal(f.store.pairing, undefined);
});

test("forged senders and expired pairings never get cookies", async () => {
  const f = fixture(), handle = createBridge(f.api, f.request, () => 1000);
  await assert.rejects(handle({type: "PAIR"}, {...f.page, frameId: 1}), /connection page/);
  await assert.rejects(handle({type: "PAIR"}, {...f.page, id: "other"}), /sender/);
  await assert.rejects(handle({type: "CONNECT"}, f.popup), /first/);
  f.store.pairing = {...pairingFromUrl(url, 0), expires: 1};
  await assert.rejects(handle({type: "CONNECT"}, f.popup), /first/);
  assert.equal(f.reads.length, 0);
  assert.equal(innaCookie({domain: ".google.com"}), false);
  assert.equal(innaCookie({domain: "evilinna.is"}), false);
});

test("failed validation retains pairing for another explicit attempt", async () => {
  const f = fixture();
  const handle = createBridge(f.api, async (target, options) => target.endsWith("/info") ? f.request(target, options) : {status: 422}, () => 1000);
  await handle({type: "PAIR"}, f.page);
  await assert.rejects(handle({type: "CONNECT"}, f.popup), /Finish signing in/);
  assert.ok(f.store.pairing);
});
