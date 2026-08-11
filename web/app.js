const screen = document.getElementById("screen");
const dialog = document.getElementById("add-account");

const SERIES = 8;
const STATE_LABEL = {
  active: "Active",
  rate_limited: "Rate limited",
  sign_in_expired: "Sign-in expired",
  disabled: "Disabled",
};

const state = { overview: null, accounts: [], requests: [], models: [], users: [], keys: [], signin: [], locked: false };

async function api(path, options) {
  const response = await fetch(path, {
    headers: { "content-type": "application/json" },
    ...options,
  });
  if (response.status === 401) {
    state.locked = true;
    throw new Error("locked");
  }
  if (response.status === 204) return null;
  const body = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(body.error ?? `request failed (${response.status})`);
  return body;
}

function el(tag, attrs = {}, children = []) {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(attrs)) {
    if (value === null || value === undefined || value === false) continue;
    if (key === "class") node.className = value;
    else if (key === "text") node.textContent = value;
    else if (key.startsWith("on")) node.addEventListener(key.slice(2), value);
    else node.setAttribute(key, value);
  }
  for (const child of [].concat(children)) {
    if (child === null || child === undefined) continue;
    node.append(child.nodeType ? child : document.createTextNode(String(child)));
  }
  return node;
}

function seriesColor(index) {
  return `var(--series-${(index % SERIES) + 1})`;
}

function dot(index) {
  return el("span", { class: "dot", style: `background:${seriesColor(index)}` });
}

function unitFor(values) {
  const max = Math.max(0, ...values.filter((value) => Number.isFinite(value)));
  if (max >= 1_000_000) return { divisor: 1_000_000, suffix: "M" };
  if (max >= 1_000) return { divisor: 1_000, suffix: "K" };
  return { divisor: 1, suffix: "" };
}

function fixed(value, unit, decimals = 2) {
  if (value === null || value === undefined) return "—";
  if (unit.divisor === 1) return String(value);
  return (value / unit.divisor).toFixed(decimals) + unit.suffix;
}

function money(micros) {
  if (micros === null || micros === undefined) return "—";
  return `$${(micros / 1_000_000).toFixed(2)}`;
}

function ms(value) {
  if (value === null || value === undefined) return "—";
  return value >= 1000 ? `${(value / 1000).toFixed(1)}s` : `${value}ms`;
}

function clock(epochMs) {
  return new Date(epochMs).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  });
}

function stateBadge(value) {
  return el("span", { class: `state ${value}` }, [
    el("span", { class: "dot" }),
    STATE_LABEL[value] ?? value,
  ]);
}

function empty(title, next) {
  return el("div", { class: "empty" }, [el("strong", { text: title }), next]);
}

function table(columns, rows) {
  return el("table", {}, [
    el("thead", {}, [
      el(
        "tr",
        {},
        columns.map((column) =>
          el("th", { class: column.num ? "num" : null, text: column.label ?? "" }),
        ),
      ),
    ]),
    el("tbody", {}, rows),
  ]);
}

function unlocked() {
  return el("section", {}, [
    el("h1", { text: "goat-gateway" }),
    el("div", { class: "empty" }, [
      el("strong", { text: "An admin key is required." }),
      el("div", { class: "muted" }, [
        "It was printed to the terminal the first time this server started.",
      ]),
      el("form", {
        class: "next",
        style: "display:flex;gap:8px;max-width:420px;margin:14px auto 0",
        onsubmit: async (event) => {
          event.preventDefault();
          const field = event.target.key;
          try {
            await fetch("/api/session", {
              method: "POST",
              headers: { "content-type": "application/json" },
              body: JSON.stringify({ key: field.value.trim() }),
            }).then((response) => {
              if (!response.ok) throw new Error("that key does not match");
            });
            state.locked = false;
            refresh();
          } catch (failure) {
            document.getElementById("lock-error").textContent = failure.message;
          }
        },
      }, [
        el("input", { name: "key", placeholder: "gwa_…", autocomplete: "off" }),
        el("button", { text: "Enter" }),
      ]),
      el("p", { class: "error", id: "lock-error" }),
    ]),
  ]);
}

function render() {
  if (state.locked) {
    screen.replaceChildren(unlocked());
    return;
  }
  const route = (location.hash || "#/overview").slice(2);
  for (const link of document.querySelectorAll("nav a")) {
    link.toggleAttribute("aria-current", link.getAttribute("href") === `#/${route}`);
    if (link.getAttribute("href") === `#/${route}`) link.setAttribute("aria-current", "page");
  }
  const screens = { overview, accounts, requests, usage, connect, settings };
  screen.replaceChildren((screens[route] ?? overview)());
}

function overview() {
  const providers = state.overview?.providers ?? [];
  if (!state.accounts.length) {
    return el("section", {}, [
      el("h1", { text: "Overview" }),
      empty(
        "No accounts registered.",
        el("div", { class: "next" }, [
          el("button", { onclick: openAddAccount, text: "Add account" }),
        ]),
      ),
    ]);
  }

  const broken = state.accounts.filter((account) => account.state !== "active");
  const totals = totalsByProvider();
  const unit = unitFor(providers.map((provider) => totals[provider.provider]?.tokens ?? 0));

  const rows = providers.map((provider, index) => {
    const total = totals[provider.provider] ?? { tokens: 0, cost: 0, errors: 0, requests: 0 };
    return el("tr", {}, [
      el("td", {}, [dot(index), provider.provider]),
      el("td", { class: "muted", text: describeProvider(provider) }),
      el("td", { class: "num", text: fixed(total.tokens, unit) }),
      el("td", { class: "num", text: total.cost ? money(total.cost) : "—" }),
      el("td", {
        class: "num",
        text: total.requests ? `${((total.errors / total.requests) * 100).toFixed(1)}%` : "—",
      }),
    ]);
  });

  return el("section", {}, [
    el("h1", { text: "Overview" }),
    ...broken.map((account) =>
      el("div", { class: "banner" }, [
        el("span", { class: "dot" }),
        `${account.name} — ${STATE_LABEL[account.state]}`,
      ]),
    ),
    el("div", { class: "between" }, [
      el("div", { class: "row" }, [
        el("span", { class: "muted", text: "Last 200 requests" }),
        el("span", { class: "live", text: `${inFlightLabel()}` }),
      ]),
    ]),
    chart(),
    table(
      [
        { label: "Provider" },
        { label: "Accounts" },
        { label: "Tokens", num: true },
        { label: "Cost", num: true },
        { label: "Errors", num: true },
      ],
      rows,
    ),
  ]);
}

function describeProvider(provider) {
  const base = `${provider.accounts} account${provider.accounts === 1 ? "" : "s"} · ${provider.usable} usable`;
  if (provider.soonest_reset_ms) {
    return `${base} · next free at ${clock(provider.soonest_reset_ms)}`;
  }
  return base;
}

function inFlightLabel() {
  return `${state.requests.length} recorded`;
}

function totalsByProvider() {
  const totals = {};
  for (const request of state.requests) {
    const bucket = (totals[request.provider] ??= {
      tokens: 0,
      cost: 0,
      errors: 0,
      requests: 0,
    });
    const usage = request.usage ?? {};
    bucket.tokens +=
      (usage.input_tokens ?? 0) + (usage.output_tokens ?? 0) + (usage.cache_read_tokens ?? 0);
    bucket.cost += request.cost_micros ?? 0;
    bucket.requests += 1;
    if (request.status !== "ok") bucket.errors += 1;
  }
  return totals;
}

function chart() {
  const buckets = new Array(48).fill(0);
  if (!state.requests.length) {
    return el("div", { class: "chart" });
  }
  const newest = state.requests[0].started_at;
  const oldest = state.requests[state.requests.length - 1].started_at;
  const span = Math.max(1, newest - oldest);
  for (const request of state.requests) {
    const slot = Math.min(
      buckets.length - 1,
      Math.floor(((request.started_at - oldest) / span) * (buckets.length - 1)),
    );
    buckets[slot] += 1;
  }
  const peak = Math.max(...buckets, 1);
  return el(
    "div",
    { class: "chart" },
    buckets.map((count) =>
      el("div", {
        class: "bar",
        style: `height:${Math.max(2, (count / peak) * 100)}%`,
      }),
    ),
  );
}

function accounts() {
  if (!state.accounts.length) {
    return el("section", {}, [
      el("h1", { text: "Accounts" }),
      empty(
        "No accounts registered.",
        el("div", { class: "next" }, [
          el("button", { onclick: openAddAccount, text: "Add account" }),
        ]),
      ),
    ]);
  }

  const providers = [...new Set(state.accounts.map((account) => account.provider))];
  return el("section", {}, [
    el("div", { class: "between" }, [
      el("h1", { text: "Accounts", style: "margin:0" }),
      el("button", { onclick: openAddAccount, text: "Add account" }),
    ]),
    ...providers.map((provider) => {
      const mine = state.accounts.filter((account) => account.provider === provider);
      const rows = mine.map((account) =>
        el("tr", {}, [
          el("td", { text: account.name }),
          el("td", {}, [stateBadge(account.state)]),
          el("td", { class: "num muted", text: limitsFor(account.name) }),
          el("td", { class: "num" }, [actionsFor(account)]),
        ]),
      );
      return el("div", { class: "group" }, [
        el("h2", { text: provider }),
        table(
          [{ label: "Name" }, { label: "State" }, { label: "Limit", num: true }, { label: "" }],
          rows,
        ),
      ]);
    }),
  ]);
}

function limitsFor(name) {
  const providers = state.overview?.providers ?? [];
  for (const provider of providers) {
    const found = (provider.limits ?? []).find((limit) => limit.account === name);
    if (!found || !found.windows.length) continue;
    return found.windows
      .map((window) => `${window.label} ${Math.round(window.used_percent)}%`)
      .join(" · ");
  }
  return "—";
}

function actionsFor(account) {
  if (account.state === "sign_in_expired") {
    return el("button", { text: "Sign in again", onclick: () => setState(account.name, "active") });
  }
  const next = account.state === "disabled" ? "active" : "disabled";
  return el("span", { class: "row" }, [
    el("button", {
      class: "quiet",
      text: next === "disabled" ? "Turn off" : "Turn on",
      onclick: () => setState(account.name, next),
    }),
    el("button", {
      class: "quiet",
      text: "Remove",
      onclick: () => removeAccount(account.name),
    }),
  ]);
}

function requests() {
  if (!state.requests.length) {
    return el("section", {}, [
      el("h1", { text: "Requests" }),
      empty(
        "No requests yet.",
        el("div", { class: "next muted" }, ["Point a tool at this gateway from Connect."]),
      ),
    ]);
  }

  const unit = unitFor(state.requests.map(totalTokens));
  const rows = state.requests.map((request) =>
    el("tr", { onclick: () => showRequest(request), style: "cursor:pointer" }, [
      el("td", { class: "mono", text: clock(request.started_at) }),
      el("td", {}, [dot(providerIndex(request.provider)), request.model]),
      el("td", { class: "muted", text: request.account ?? "—" }),
      el("td", {}, [
        request.status === "ok"
          ? el("span", { class: "muted", text: request.translated ? "translated" : "passthrough" })
          : el("span", { style: "color:var(--critical)", text: request.error_kind ?? "error" }),
      ]),
      el("td", { class: "num", text: ms(request.ttft_ms) }),
      el("td", { class: "num", text: fixed(totalTokens(request), unit) }),
      el("td", { class: "num", text: money(request.cost_micros) }),
    ]),
  );

  return el("section", {}, [
    el("h1", { text: "Requests" }),
    table(
      [
        { label: "Time" },
        { label: "Model" },
        { label: "Account" },
        { label: "Path" },
        { label: "TTFT", num: true },
        { label: "Tokens", num: true },
        { label: "Cost", num: true },
      ],
      rows,
    ),
  ]);
}

function totalTokens(request) {
  const usage = request.usage ?? {};
  return (usage.input_tokens ?? 0) + (usage.output_tokens ?? 0) + (usage.cache_read_tokens ?? 0);
}

function providerIndex(provider) {
  const providers = [...new Set(state.accounts.map((account) => account.provider))];
  const index = providers.indexOf(provider);
  return index < 0 ? 0 : index;
}

function showRequest(request) {
  const identical = request.byte_identical;
  const verdict =
    identical === true
      ? el("span", { class: "verdict", text: "byte-identical" })
      : el("span", { class: "muted", text: "translated" });

  screen.replaceChildren(
    el("section", { class: "detail" }, [
      el("div", { class: "between" }, [
        el("h1", { text: request.model, style: "margin:0" }),
        el("button", { text: "Back", onclick: render }),
      ]),
      el("dl", {}, [
        el("dt", { text: "Path" }),
        el("dd", {}, [`${request.ingress} → ${request.egress}  `, verdict]),
        el("dt", { text: "Account" }),
        el("dd", { text: request.account ?? "—" }),
        el("dt", { text: "Digests" }),
        el("dd", { text: `${request.input_digest ?? "—"}  →  ${request.output_digest ?? "—"}` }),
        el("dt", { text: "Timing" }),
        el("dd", { text: `first byte ${ms(request.ttft_ms)}` }),
        el("dt", { text: "Tracing" }),
        el("dd", {
          text: `${request.id}${
            request.upstream_request_id ? `  ·  upstream ${request.upstream_request_id}` : ""
          }`,
        }),
        el("dt", { text: "What we changed" }),
        el("dd", {}, [
          el("div", {
            class: "evidence",
            text: request.evidence
              ? JSON.stringify(request.evidence, null, 2)
              : "nothing recorded",
          }),
        ]),
      ]),
    ]),
  );
}

function usage() {
  const byModel = {};
  for (const request of state.requests) {
    const bucket = (byModel[request.model] ??= { requests: 0, tokens: 0, cache: 0, cost: 0 });
    bucket.requests += 1;
    bucket.tokens += totalTokens(request);
    bucket.cache += request.usage?.cache_read_tokens ?? 0;
    bucket.cost += request.cost_micros ?? 0;
  }
  const models = Object.entries(byModel);
  if (!models.length) {
    return el("section", {}, [el("h1", { text: "Usage" }), empty("Nothing recorded yet.", null)]);
  }

  const unit = unitFor(models.map(([, value]) => value.tokens));
  const rows = models.map(([model, value], index) =>
    el("tr", {}, [
      el("td", {}, [dot(index), model]),
      el("td", { class: "num", text: value.requests }),
      el("td", { class: "num", text: fixed(value.tokens, unit) }),
      el("td", { class: "num", text: fixed(value.cache, unit) }),
      el("td", { class: "num", text: value.cost ? money(value.cost) : "—" }),
    ]),
  );

  return el("section", {}, [
    el("h1", { text: "Usage" }),
    table(
      [
        { label: "Model" },
        { label: "Requests", num: true },
        { label: "Tokens", num: true },
        { label: "Cache read", num: true },
        { label: "Cost", num: true },
      ],
      rows,
    ),
    el("p", { class: "muted", style: "font-size:var(--small)" }, [
      `Cost is estimated from the price table (${state.overview?.pricing_as_of ?? "unknown"}). Models with no price show —.`,
    ]),
  ]);
}

function connect() {
  const base = location.origin;
  const codex = `[model_providers.goat]
name = "goat-gateway"
base_url = "${base}/v1"
wire_api = "responses"
env_key = "GOAT_API_KEY"`;
  const claudeCode = `export ANTHROPIC_BASE_URL=${base}
export ANTHROPIC_AUTH_TOKEN=<your key>`;

  return el("section", {}, [
    el("h1", { text: "Connect" }),
    el("div", { class: "group" }, [
      el("h2", { text: "Codex" }),
      el("div", { class: "evidence", text: codex }),
    ]),
    el("div", { class: "group" }, [
      el("h2", { text: "Claude Code" }),
      el("div", { class: "evidence", text: claudeCode }),
    ]),
    el("p", { class: "muted", style: "font-size:var(--small)" }, [
      "Codex speaks the Responses API; Claude Code speaks Anthropic Messages. Both are served here.",
    ]),
  ]);
}

function settings() {
  const theme = document.documentElement.dataset.theme;
  return el("section", {}, [
    el("h1", { text: "Settings" }),
    el("div", { class: "group" }, [
      el("h2", { text: "Appearance" }),
      el("div", { class: "row", style: "padding:0 12px" }, [
        el("button", {
          text: theme === "dark" ? "Switch to light" : "Switch to dark",
          onclick: () => {
            const next = theme === "dark" ? "light" : "dark";
            document.documentElement.dataset.theme = next;
            localStorage.setItem("theme", next);
            render();
          },
        }),
      ]),
    ]),
    el("div", { class: "group" }, [
      el("div", { class: "between", style: "padding:0 12px" }, [
        el("h2", { text: "Users", style: "margin:0" }),
        el("button", { text: "Add user", onclick: addUser }),
      ]),
      usersTable(),
    ]),
    el("div", { class: "group" }, [
      el("h2", { text: "Models" }),
      el("div", { class: "evidence", text: state.models.join("\n") || "none" }),
    ]),
    el("div", { class: "group" }, [
      el("h2", { text: "Session" }),
      el("div", { style: "padding:0 12px" }, [
        el("button", {
          text: "Sign out",
          onclick: async () => {
            await fetch("/api/session", { method: "DELETE" });
            state.locked = true;
            render();
          },
        }),
      ]),
    ]),
  ]);
}

function usersTable() {
  if (!state.users.length) {
    return el("div", { class: "empty" }, [
      el("strong", { text: "No users yet." }),
      el("div", { class: "muted" }, ["A user owns the API keys their tools use."]),
    ]);
  }

  const rows = [];
  for (const user of state.users) {
    const theirs = state.keys.filter((key) => key.user_id === user.id);
    rows.push(
      el("tr", {}, [
        el("td", { text: user.name }),
        el("td", { class: "muted", text: `${theirs.length} key${theirs.length === 1 ? "" : "s"}` }),
        el("td", { class: "num" }, [
          el("span", { class: "row", style: "justify-content:flex-end" }, [
            el("button", { class: "quiet", text: "New key", onclick: () => addKey(user) }),
            el("button", { class: "quiet", text: "Remove", onclick: () => removeUser(user.id) }),
          ]),
        ]),
      ]),
    );
    for (const key of theirs) {
      rows.push(
        el("tr", {}, [
          el("td", { class: "muted", style: "padding-left:28px", text: key.label }),
          el("td", { class: "mono muted", text: `${key.prefix}…` }),
          el("td", { class: "num" }, [
            el("span", { class: "row", style: "justify-content:flex-end;gap:12px" }, [
              el("span", {
                class: "muted",
                style: "font-size:var(--small)",
                text: key.revoked_at
                  ? "revoked"
                  : key.last_used_at
                    ? `used ${clock(key.last_used_at)}`
                    : "never used",
              }),
              key.revoked_at
                ? null
                : el("button", {
                    class: "quiet",
                    text: "Revoke",
                    onclick: () => revokeKey(key.id),
                  }),
            ]),
          ]),
        ]),
      );
    }
  }
  return table([{ label: "User" }, { label: "" }, { label: "" }], rows);
}

async function addUser() {
  const name = prompt("Name");
  if (!name?.trim()) return;
  try {
    await api("/api/users", { method: "POST", body: JSON.stringify({ name: name.trim() }) });
    await refresh();
  } catch (failure) {
    alert(failure.message);
  }
}

async function removeUser(id) {
  await api(`/api/users/${encodeURIComponent(id)}`, { method: "DELETE" });
  await refresh();
}

async function addKey(user) {
  const label = prompt(`Label for ${user.name}'s new key`, "맥북 Codex");
  if (!label?.trim()) return;
  const issued = await api("/api/keys", {
    method: "POST",
    body: JSON.stringify({ user_id: user.id, label: label.trim() }),
  });
  await refresh();
  screen.prepend(
    el("div", { class: "banner" }, [
      el("div", {}, [
        el("div", { text: "Copy this now — it is not shown again." }),
        el("div", { class: "mono", style: "margin-top:6px", text: issued.key }),
      ]),
    ]),
  );
}

async function revokeKey(id) {
  await api(`/api/keys/${encodeURIComponent(id)}`, { method: "DELETE" });
  await refresh();
}

function openAddAccount() {
  document.getElementById("add-account-error").hidden = true;
  syncAddAccountForm();
  dialog.showModal();
}

const MODE_LABELS = {
  loopback: "Browser on this machine",
  paste: "Browser, then paste a code",
  device: "Device code, for a machine with no browser",
};

function modesFor(provider) {
  return (state.signin.find((entry) => entry.provider === provider) || {}).modes || [];
}

function syncAddAccountForm() {
  const form = document.getElementById("add-account-form");
  const wantsSignIn = form.method.value === "signin";
  const modes = modesFor(form.provider.value);

  document.getElementById("account-key-fields").hidden = wantsSignIn;
  document.getElementById("account-signin-fields").hidden = !wantsSignIn;
  document.getElementById("account-mode").replaceChildren(
    ...modes.map((mode) => el("option", { value: mode, text: MODE_LABELS[mode] || mode })),
  );

  const impossible = wantsSignIn && !modes.length;
  document.getElementById("add-account-save").disabled = impossible;
  if (impossible) {
    const error = document.getElementById("add-account-error");
    error.textContent = `${form.provider.value} has no sign-in flow on this gateway. Use an API key.`;
    error.hidden = false;
  }
}

document.getElementById("account-method").addEventListener("change", syncAddAccountForm);
document.getElementById("account-provider").addEventListener("change", syncAddAccountForm);

document.getElementById("add-account-form").addEventListener("submit", async (event) => {
  const form = event.target;
  if (event.submitter?.value !== "save") return;
  event.preventDefault();

  const error = document.getElementById("add-account-error");
  try {
    if (form.method.value === "signin") {
      const started = await api("/api/signin", {
        method: "POST",
        body: JSON.stringify({
          provider: form.provider.value,
          name: form.name.value.trim(),
          mode: form.mode.value,
        }),
      });
      form.reset();
      dialog.close();
      openSignIn(started);
      return;
    }

    await api("/api/accounts", {
      method: "POST",
      body: JSON.stringify({
        name: form.name.value.trim(),
        provider: form.provider.value,
        secret: form.secret.value.trim(),
      }),
    });
    form.reset();
    dialog.close();
    await refresh();
  } catch (failure) {
    error.textContent = failure.message;
    error.hidden = false;
  }
});

const signInDialog = document.getElementById("signin");
let signInWatch = null;

function openSignIn(started) {
  const detail = document.getElementById("signin-detail");
  const pasteBlock = document.getElementById("signin-paste-block");
  const codeBlock = document.getElementById("signin-code-block");
  const finish = document.getElementById("signin-finish");
  const error = document.getElementById("signin-error");

  error.hidden = true;
  pasteBlock.hidden = started.mode !== "paste";
  finish.hidden = started.mode !== "paste";
  codeBlock.hidden = started.mode !== "device";
  document.getElementById("signin-pasted").value = "";

  if (started.authorize_url) window.open(started.authorize_url, "_blank", "noopener");

  if (started.mode === "device") {
    document.getElementById("signin-user-code").textContent = started.user_code;
    detail.textContent = `Open ${started.verification_url} on any device and enter this code.`;
  } else if (started.mode === "paste") {
    detail.textContent =
      "Finish in your browser, then paste the code it shows you.";
  } else {
    detail.textContent = "Finish in your browser. This page moves on by itself.";
  }

  signInDialog.showModal();

  if (started.mode !== "paste") watchSignIn(started.session);
  finish.onclick = () => finishPaste(started.session);
}

function watchSignIn(session) {
  stopWatching();
  signInWatch = setInterval(async () => {
    let status;
    try {
      status = await api(`/api/signin/${encodeURIComponent(session)}`);
    } catch {
      return;
    }
    if (status.status === "done") {
      stopWatching();
      signInDialog.close();
      await refresh();
    } else if (status.status === "failed") {
      stopWatching();
      const error = document.getElementById("signin-error");
      error.textContent = status.message;
      error.hidden = false;
    }
  }, 1500);
}

function stopWatching() {
  if (signInWatch) clearInterval(signInWatch);
  signInWatch = null;
}

async function finishPaste(session) {
  const error = document.getElementById("signin-error");
  try {
    await api(`/api/signin/${encodeURIComponent(session)}`, {
      method: "POST",
      body: JSON.stringify({ code: document.getElementById("signin-pasted").value }),
    });
    signInDialog.close();
    await refresh();
  } catch (failure) {
    error.textContent = failure.message;
    error.hidden = false;
  }
}

document.getElementById("signin-cancel").addEventListener("click", () => {
  stopWatching();
  signInDialog.close();
});

async function setState(name, next) {
  await api(`/api/accounts/${encodeURIComponent(name)}/state`, {
    method: "POST",
    body: JSON.stringify({ state: next }),
  });
  await refresh();
}

async function removeAccount(name) {
  await api(`/api/accounts/${encodeURIComponent(name)}`, { method: "DELETE" });
  await refresh();
}

async function refresh() {
  const [overviewData, accountsData, requestsData, modelsData, usersData, keysData, signinData] =
    await Promise.all([
      api("/api/overview").catch(() => null),
      api("/api/accounts").catch(() => ({ accounts: [] })),
      api("/api/requests").catch(() => ({ requests: [] })),
      api("/api/models").catch(() => ({ anthropic: [] })),
      api("/api/users").catch(() => ({ users: [] })),
      api("/api/keys").catch(() => ({ keys: [] })),
      api("/api/signin/providers").catch(() => ({ providers: [] })),
    ]);
  if (state.locked) {
    render();
    return;
  }
  state.overview = overviewData;
  state.accounts = accountsData.accounts ?? [];
  state.requests = requestsData.requests ?? [];
  state.models = modelsData.anthropic ?? [];
  state.users = usersData.users ?? [];
  state.keys = keysData.keys ?? [];
  state.signin = signinData.providers ?? [];
  syncProviderChoices();
  render();
}

function syncProviderChoices() {
  const select = document.getElementById("account-provider");
  const known = new Set(state.signin.map((entry) => entry.provider));
  for (const option of select.options) known.delete(option.value);
  for (const provider of known) {
    select.append(el("option", { value: provider, text: provider }));
  }
}

const saved = localStorage.getItem("theme");
if (saved) document.documentElement.dataset.theme = saved;

window.addEventListener("hashchange", render);
refresh();
setInterval(refresh, 4000);
