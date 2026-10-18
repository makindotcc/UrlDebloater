export const DEFAULT_MIXER_URL = "https://urldebloater.makin.cc/";
export const DEFAULT_SETTINGS = {
  enabled: true,
  redirectPolicy: "local",
  mixerUrl: DEFAULT_MIXER_URL,
};

export function getSettings() {
  return chrome.storage.sync.get(DEFAULT_SETTINGS);
}
