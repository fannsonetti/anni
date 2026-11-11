// Runs only in Anni's own persistent profile. Never reads or fills credentials.
(() => {
    if (window.top !== window || location.protocol !== "https:") return;
    const silent = __ANNI_SILENT__;
    if (location.hostname === "accounts.google.com") {
        const key = "anni.lastGoogleAccount";
        const account = element => element?.closest("[data-identifier], [data-email]");
        const identifier = element => element?.getAttribute("data-identifier") || element?.getAttribute("data-email");
        const read = () => { try { return localStorage.getItem(key); } catch (_) { return null; } };
        const remember = value => { try { localStorage.setItem(key, value); } catch (_) {} };
        // Remember the account the user chooses, so multiple remembered accounts stay unambiguous.
        document.addEventListener("click", event => {
            const value = identifier(account(event.target));
            if (value?.includes("@")) remember(value);
        }, true);
        if (!silent) return;
        let selected = false;
        const choose = () => {
            if (selected) return;
            const rows = [...document.querySelectorAll("[data-identifier], [data-email]")]
                .filter(row => identifier(row)?.includes("@") && row.getClientRects().length);
            const previous = read();
            const accounts = [...new Set(rows.map(identifier))];
            const wanted = previous || (accounts.length === 1 ? accounts[0] : null);
            const row = wanted && rows.find(row => identifier(row) === wanted);
            if (row) {
                selected = true;
                row.click();
            }
        };
        setInterval(choose, 500);
        return;
    }
    if (!silent || !["nam.inna.is", "r.inna.is"].includes(location.hostname)) return;
    const resume = () => {
        // Use INNA's own Google button, retaining any callback parameters in its link.
        // INNA renders a hidden duplicate for its other layout; use the visible button.
        const button = [...document.querySelectorAll(".btn-opacity-google")].find(b => b.getClientRects().length);
        if (!button) return;
        try {
            if (sessionStorage.getItem("anni.googleResumeStarted")) return;
            sessionStorage.setItem("anni.googleResumeStarted", "true");
        } catch (_) { return; }
        (button.querySelector('a[href^="/auth/google"]') || button).click();
    };
    setInterval(resume, 500);
})();
