import init, { UrlCleaner } from "./wasm/urldebloater_urlwasher_wasm.js";
import { DEFAULT_MIXER_URL, getSettings } from "./settings.js";

const isFirefox = globalThis.browser?.runtime?.getBrowserInfo !== undefined;
const runtime = isFirefox ? globalThis.browser : chrome;

const SHORT_LINK_HOSTS = ["vm.tiktok.com", "on.soundcloud.com"];
const SHORT_LINK_GATE_RULE_ID = 1;
const WAITING_PAGE_PATH = "wait.html";
const WAITING_PAGE_FAILURE_HASH = "#failed";
const SHORT_LINK_GATE_RULE = {
  id: SHORT_LINK_GATE_RULE_ID,
  priority: 1,
  action: {
    type: "redirect",
    redirect: { extensionPath: `/${WAITING_PAGE_PATH}` },
  },
  condition: {
    requestDomains: SHORT_LINK_HOSTS,
    resourceTypes: ["main_frame"],
  },
};

let wasmPromise;
let cleanerPromise;
let cleanerMixerUrl;

function ensureWasm() {
  if (!wasmPromise) {
    wasmPromise = init(
      runtime.runtime.getURL("wasm/urldebloater_urlwasher_wasm_bg.wasm"),
    );
  }
  return wasmPromise;
}

function getCleaner(mixerUrl) {
  if (!cleanerPromise || cleanerMixerUrl !== mixerUrl) {
    cleanerMixerUrl = mixerUrl;
    cleanerPromise = (async () => {
      await ensureWasm();
      try {
        return new UrlCleaner(mixerUrl);
      } catch {
        return new UrlCleaner(DEFAULT_MIXER_URL);
      }
    })();
  }
  return cleanerPromise;
}

async function cleanUrl(url, settings) {
  const { enabled, redirectPolicy, mixerUrl } = settings ?? (await getSettings());
  if (!enabled) return url;

  const cleaner = await getCleaner(mixerUrl);
  if (redirectPolicy === "mixer") return cleaner.wash(url);
  if (redirectPolicy === "local") {
    return cleaner.wash_with_local_redirects(url);
  }
  return cleaner.wash_without_short_links(url);
}

function updateShortLinkGate(enabled, redirectPolicy) {
  const addRules = enabled && redirectPolicy !== "off" ? [SHORT_LINK_GATE_RULE] : [];
  return chrome.declarativeNetRequest.updateDynamicRules({
    addRules,
    removeRuleIds: [SHORT_LINK_GATE_RULE_ID],
  });
}

async function syncShortLinkGate() {
  const { enabled, redirectPolicy } = await getSettings();
  await updateShortLinkGate(enabled, redirectPolicy);
}

function isShortLink(url) {
  try {
    return SHORT_LINK_HOSTS.includes(new URL(url).hostname);
  } catch {
    return false;
  }
}

function showGateFailure(tabId) {
  return chrome.tabs.update(tabId, {
    url: chrome.runtime.getURL(`${WAITING_PAGE_PATH}${WAITING_PAGE_FAILURE_HASH}`),
  });
}

async function handleChromeNavigation(details) {
  const settings = await getSettings();
  const gated =
    settings.enabled && settings.redirectPolicy !== "off" && isShortLink(details.url);
  try {
    const washed = await cleanUrl(details.url, settings);
    if (washed && washed !== details.url) {
      await chrome.tabs.update(details.tabId, { url: washed });
    } else if (gated) {
      await showGateFailure(details.tabId);
    }
  } catch {
    if (gated) {
      await showGateFailure(details.tabId).catch(() => {});
    }
  }
}

if (isFirefox) {
  runtime.webRequest.onBeforeRequest.addListener(
    async (details) => {
      try {
        const washed = await cleanUrl(details.url);
        if (washed && washed !== details.url) {
          return { redirectUrl: washed };
        }
      } catch {}
      return {};
    },
    {
      urls: ["http://*/*", "https://*/*"],
      types: ["main_frame"],
    },
    ["blocking"],
  );
} else {
  syncShortLinkGate().catch(() => {});
  chrome.storage.onChanged.addListener((changes, areaName) => {
    if (areaName === "sync" && (changes.enabled || changes.redirectPolicy)) {
      syncShortLinkGate().catch(() => {});
    }
  });
  chrome.webRequest.onBeforeRequest.addListener(
    (details) => {
      if (details.frameId !== 0 || details.tabId < 0) return;

      handleChromeNavigation(details).catch(() => {});
    },
    {
      urls: ["http://*/*", "https://*/*"],
      types: ["main_frame"],
    },
  );
}
