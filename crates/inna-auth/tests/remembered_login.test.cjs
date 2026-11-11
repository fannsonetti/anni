const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const source = fs.readFileSync(require('node:path').join(__dirname, '../src/remembered_login.js'), 'utf8');

function page({ host = 'accounts.google.com', silent = true, previous, emails = [], protocol = 'https:', frame = false } = {}) {
    const values = new Map(previous ? [['anni.lastGoogleAccount', previous]] : []);
    const listeners = {};
    const timers = [];
    const clicks = [];
    const rows = emails.map(email => ({
        getAttribute: name => name === 'data-identifier' ? email : null,
        getClientRects: () => [1],
        closest() { return this; },
        click() { clicks.push(email); listeners.click?.({ target: this }); },
    }));
    const link = { click: () => clicks.push('google-link-with-callback') };
    const hidden = { getClientRects: () => [], querySelector: () => ({ click: () => clicks.push("hidden-button") }) };
    const button = { getClientRects: () => [1], querySelector: () => link };
    const storage = { getItem: key => values.get(key), setItem: (key, value) => values.set(key, value) };
    const window = {};
    window.top = frame ? {} : window;
    vm.runInNewContext(source.replace('__ANNI_SILENT__', String(silent)), {
        window, location: { hostname: host, protocol },
        document: { addEventListener: (type, handler) => listeners[type] = handler, querySelectorAll: selector => selector === ".btn-opacity-google" ? [hidden, button] : rows },
        localStorage: storage, sessionStorage: storage, setInterval: callback => timers.push(callback),
    });
    return { clicks, values, rows, tick: () => timers.forEach(callback => callback()) };
}

test('reuses the last selected account when several Google accounts are remembered', () => {
    const p = page({ emails: ['first@example.test', 'last@example.test'], previous: 'last@example.test' });
    p.tick(); p.tick();
    assert.deepEqual(p.clicks, ['last@example.test']);
});
test('uses a sole remembered account, including duplicate chooser elements', () => {
    const p = page({ emails: ['only@example.test', 'only@example.test'] });
    p.tick();
    assert.deepEqual(p.clicks, ['only@example.test']);
});
test('does not guess among multiple accounts or substitute another for a missing account', () => {
    for (const previous of [undefined, 'missing@example.test']) {
        const p = page({ emails: ['one@example.test', 'two@example.test'], previous });
        p.tick();
        assert.deepEqual(p.clicks, []);
    }
});
test('interactive sign-in remembers a choice without clicking automatically', () => {
    const p = page({ silent: false, emails: ['chosen@example.test'] });
    p.tick();
    assert.deepEqual(p.clicks, []);
    p.rows[0].click();
    assert.equal(p.values.get('anni.lastGoogleAccount'), 'chosen@example.test');
});
test('INNA Google button preserves its callback and is activated only once', () => {
    for (const host of ['nam.inna.is', 'r.inna.is']) {
        const p = page({ host });
        p.tick(); p.tick();
        assert.deepEqual(p.clicks, ['google-link-with-callback']);
    }
});
test('leaves other hosts, insecure pages, frames, and password fields alone', () => {
    for (const options of [{ host: 'example.test' }, { protocol: 'http:' }, { frame: true }, { emails: ['password'] }]) {
        const p = page(options);
        p.tick();
        assert.deepEqual(p.clicks, []);
    }
});
