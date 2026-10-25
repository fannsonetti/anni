const INNA = "https://nam.inna.is/";

export function pairingFromUrl(raw, now = Date.now()) {
  let url;
  try { url = new URL(raw); } catch { return null; }
  if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.pathname !== "/connect"
      || !url.port || url.username || url.password || url.search || !/^#[a-f0-9]{64}$/.test(url.hash)) return null;
  return {origin: url.origin, token: url.hash.slice(1), expires: now + 10 * 60 * 1000};
}

export function innaCookie(cookie) {
  return cookie && ["inna.is", "nam.inna.is"].includes(cookie.domain.replace(/^\./, ""));
}

export function createBridge(api, request = fetch, now = Date.now) {
  async function localRequest(pairing, route, options = {}) {
    // Recheck stored capability before using it; never accept arbitrary destinations from messages.
    const checked = pairingFromUrl(`${pairing.origin}/connect#${pairing.token}`, now());
    if (!checked || pairing.expires <= now()) throw new Error("Connection expired. Choose Connect in browser in Anni again.");
    return request(`${checked.origin}${route}`, {...options, credentials: "omit", cache: "no-store", redirect: "error",
      signal: AbortSignal.timeout(15000), headers: {...options.headers, Authorization: `Bearer ${checked.token}`}});
  }

  return async function handle(message, sender) {
    if (sender.id !== api.runtime.id) throw new Error("Unknown sender.");
    const popup = sender.url === api.runtime.getURL("popup.html");
    if (message.type === "PAIR") {
      // Only a top-level content script on Anni's exact loopback pairing URL can pair.
      if (sender.frameId !== 0 || !sender.tab || sender.url !== sender.tab.url) throw new Error("Open the connection page from Anni.");
      const pairing = pairingFromUrl(sender.url, now());
      if (!pairing) throw new Error("Open the connection page from Anni.");
      const response = await localRequest(pairing, "/info");
      if (!response.ok || (await response.json()).protocol !== "anni-browser-connect-v1") throw new Error("Anni connection could not be verified.");
      await api.storage.session.set({pairing});
      return {ok: true};
    }
    if (!popup) throw new Error("Use the Anni Connect popup.");
    const {pairing} = await api.storage.session.get("pairing");
    if (message.type === "STATUS") return {ok: true, paired: Boolean(pairing && pairing.expires > now())};
    if (message.type !== "CONNECT") throw new Error("Unknown action.");
    if (!pairing || pairing.expires <= now()) throw new Error("In Anni, choose Connect in browser first.");
    // Read only INNA cookies on this explicit popup action. Never inspect Google cookies or browser databases.
    const cookies = (await api.cookies.getAll({url: INNA})).filter(innaCookie).map(cookie => ({
      name: cookie.name, value: cookie.value, domain: cookie.domain, path: cookie.path,
      http_only: cookie.httpOnly, secure: cookie.secure
    }));
    if (!cookies.length) throw new Error("Sign in to INNA in this browser profile, then try again.");
    const response = await localRequest(pairing, "/session", {method: "POST", headers: {"Content-Type": "application/json"}, body: JSON.stringify({cookies})});
    if (response.status === 422) throw new Error("INNA could not verify this login. Finish signing in on INNA, then try again.");
    if (response.status !== 204) throw new Error("Anni could not accept this session. Start a new connection in Anni.");
    await api.storage.session.remove("pairing");
    return {ok: true, connected: true};
  };
}
