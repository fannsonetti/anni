import {createBridge} from "./bridge.mjs";
const handle = createBridge(chrome);
chrome.runtime.onMessage.addListener((message, sender, respond) => {
  handle(message, sender).then(respond).catch(error => {
    // No cookie values, request bodies or exceptions are logged.
    const known = error instanceof TypeError ? "Cannot reach Anni. Choose Connect in browser in the app and try again." : error.message;
    respond({ok: false, error: known || "Could not connect. Please try again."});
  });
  return true;
});
