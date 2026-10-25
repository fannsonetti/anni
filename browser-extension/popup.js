"use strict";
const button = document.getElementById("connect");
const status = document.getElementById("status");
chrome.runtime.sendMessage({type: "STATUS"}).then(result => {
  button.disabled = !result.paired;
  status.textContent = result.paired ? "Ready. Check that INNA shows the account you want, then connect." : "In Anni, choose Connect in browser first.";
}).catch(() => { status.textContent = "Reopen this popup to try again."; });
button.addEventListener("click", async () => {
  button.disabled = true;
  status.textContent = "Checking your INNA session…";
  try {
    const result = await chrome.runtime.sendMessage({type: "CONNECT"});
    status.textContent = result.ok ? "Connected. Return to Anni." : result.error;
    button.disabled = result.ok;
  } catch { status.textContent = "Could not connect. Check that Anni is running and try again."; button.disabled = false; }
});
