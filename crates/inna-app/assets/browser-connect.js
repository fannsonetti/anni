"use strict";
const token = location.hash.slice(1);
document.getElementById("copy-folder").addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(document.getElementById("extension-folder").textContent);
    document.getElementById("copy-folder").textContent = "Copied";
  } catch { document.getElementById("copy-folder").textContent = "Select and copy the path above"; }
});
setTimeout(() => {
  if (!document.documentElement.dataset.anniExtension) {
    document.getElementById("extension-status").textContent = "Install Anni Connect once using the instructions below, then refresh this page.";
    document.getElementById("setup").open = true;
  }
}, 2000);
const poll = setInterval(async () => {
  if (!/^[a-f0-9]{64}$/.test(token)) return clearInterval(poll);
  try {
    const response = await fetch("/status", {headers: {Authorization: `Bearer ${token}`}, cache: "no-store"});
    if (!response.ok) return;
    if ((await response.json()).connected) {
      document.getElementById("connection-status").textContent = "Connected. Return to Anni; you can close this tab.";
      clearInterval(poll);
    }
  } catch {
    document.getElementById("connection-status").textContent = "This connection has finished or expired. Check Anni to continue.";
    clearInterval(poll);
  }
}, 1500);
