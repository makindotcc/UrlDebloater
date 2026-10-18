import { getSettings } from "./settings.js";

const STATUS_DISPLAY_MS = 2000;

const form = document.querySelector("#settings-form");
const enabledInput = document.querySelector("#enabled");
const mixerUrlInput = document.querySelector("#mixer-url");
const status = document.querySelector("#status");
let statusTimeout;

function message(key) {
  return chrome.i18n.getMessage(key);
}

function selectedPolicy() {
  return document.querySelector('input[name="redirectPolicy"]:checked').value;
}

function updateMixerState() {
  mixerUrlInput.disabled = selectedPolicy() !== "mixer";
}

function validMixerUrl(value) {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

async function saveSettings() {
  updateMixerState();
  const mixerUrl = mixerUrlInput.value.trim();
  const mixerUrlValid = validMixerUrl(mixerUrl);
  const settings = {
    enabled: enabledInput.checked,
    redirectPolicy: selectedPolicy(),
  };
  if (mixerUrlValid) {
    settings.mixerUrl = mixerUrl;
  }

  await chrome.storage.sync.set(settings);
  clearTimeout(statusTimeout);
  if (!mixerUrlValid) {
    status.textContent = message("mixerUrlInvalid");
    return;
  }

  status.textContent = message("settingsSaved");
  statusTimeout = setTimeout(() => {
    status.textContent = "";
  }, STATUS_DISPLAY_MS);
}

for (const element of document.querySelectorAll("[data-i18n]")) {
  element.textContent = message(element.dataset.i18n);
}

const settings = await getSettings();
enabledInput.checked = settings.enabled;
for (const input of document.querySelectorAll('input[name="redirectPolicy"]')) {
  input.checked = input.value === settings.redirectPolicy;
}
mixerUrlInput.value = settings.mixerUrl;
updateMixerState();

form.addEventListener("change", saveSettings);
form.addEventListener("submit", (event) => {
  event.preventDefault();
});
