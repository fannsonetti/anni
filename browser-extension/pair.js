"use strict";
if (window.top === window && location.pathname === "/connect" && /^#[a-f0-9]{64}$/.test(location.hash)) {
  chrome.runtime.sendMessage({type: "PAIR"}).then(result => {
    const label = document.getElementById("extension-status");
    if (!label) return;
    if (result.ok) {
      document.documentElement.dataset.anniExtension = "ready";
      label.textContent = "Anni Connect is ready. Open INNA, then click the extension to connect your account.";
    } else label.textContent = result.error;
  }).catch(() => {
    const label = document.getElementById("extension-status");
    if (label) label.textContent = "Could not pair the extension. Start a new connection from Anni.";
  });
}
