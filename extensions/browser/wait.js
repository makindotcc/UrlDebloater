const FAILURE_HASH = "#failed";

for (const element of document.querySelectorAll("[data-i18n]")) {
  element.textContent = chrome.i18n.getMessage(element.dataset.i18n);
}

const message = document.querySelector("#message");

function updateMessage() {
  const key = location.hash === FAILURE_HASH ? "waitFailed" : "waitMessage";
  message.textContent = chrome.i18n.getMessage(key);
}

updateMessage();
addEventListener("hashchange", updateMessage);
