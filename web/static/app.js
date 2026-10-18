const form = document.querySelector("#cleaner-form");
const input = document.querySelector("#wash-input");
const cleanButton = document.querySelector("#clean-button");
const unrollShortLinks = document.querySelector("#unroll-short-links");
const status = document.querySelector("#clean-status");

if (form && input && cleanButton && unrollShortLinks && status) {
    const cleanLabel = cleanButton.querySelector(".button-label");
    const defaultCleanLabel = cleanLabel.textContent;
    const defaultStatus = status.textContent;
    const configuredMixerUrl =
        document.querySelector('meta[name="mixer-url"]')?.content;
    if (!configuredMixerUrl) {
        throw new Error("Mixer URL is not configured.");
    }
    const mixerOverride = new URL(window.location.href).searchParams.get("mixer");
    const mixerUrl = mixerOverride || configuredMixerUrl;
    const unrollPreferenceKey = "urldebloater.unrollShortLinks";
    const openedFromFile = window.location.protocol === "file:";
    let cleanerPromise;
    let busy = false;

    try {
        unrollShortLinks.checked =
            window.localStorage.getItem(unrollPreferenceKey) === "true";
    } catch (error) {
        console.warn("Could not read the unroll preference.", error);
        unrollShortLinks.checked = false;
    }

    function hasText() {
        return input.value.trim().length > 0;
    }

    function setStatus(message, tone = "neutral") {
        status.textContent = message;
        status.dataset.tone = tone;
    }

    function updateControls() {
        const empty = !hasText();
        input.disabled = busy;
        unrollShortLinks.disabled = busy;
        cleanButton.disabled = busy || empty;
    }

    function setBusy(nextBusy) {
        busy = nextBusy;
        form.dataset.busy = String(busy);
        form.setAttribute("aria-busy", String(busy));
        cleanButton.classList.toggle("is-loading", busy);
        cleanLabel.textContent = busy ? "Cleaning…" : defaultCleanLabel;
        updateControls();
    }

    function loadCleaner() {
        if (!cleanerPromise) {
            cleanerPromise = (async () => {
                const wasm = await import("./wasm/urldebloater_urlwasher_wasm.js");
                await wasm.default();
                return new wasm.UrlCleaner(mixerUrl);
            })().catch((error) => {
                cleanerPromise = undefined;
                throw error;
            });
        }

        return cleanerPromise;
    }

    function startClipboardWrite(textPromise) {
        if (
            !window.isSecureContext ||
            !navigator.clipboard?.write ||
            typeof ClipboardItem === "undefined"
        ) {
            return null;
        }

        try {
            const item = new ClipboardItem({
                "text/plain": textPromise.then(
                    (text) => new Blob([text], { type: "text/plain" }),
                ),
            });
            return navigator.clipboard.write([item]);
        } catch (error) {
            console.warn("Could not start the clipboard write.", error);
            return null;
        }
    }

    async function copyText(text) {
        let clipboardError = null;
        if (navigator.clipboard?.writeText && window.isSecureContext) {
            try {
                await navigator.clipboard.writeText(text);
                return;
            } catch (error) {
                clipboardError = error;
            }
        }

        const previousFocus = document.activeElement;
        const helper = document.createElement("textarea");
        helper.className = "clipboard-helper";
        helper.value = text;
        helper.setAttribute("readonly", "");
        document.body.append(helper);
        helper.select();

        let copied = false;
        let execError = null;
        try {
            copied = document.execCommand("copy");
        } catch (error) {
            execError = error;
        } finally {
            helper.remove();
            if (previousFocus instanceof HTMLElement) {
                previousFocus.focus();
            }
        }

        if (copied) {
            if (clipboardError) {
                console.warn("Clipboard API failed; copied with the fallback.", clipboardError);
            }
            return;
        }

        throw clipboardError ?? execError ?? new Error("Clipboard copy failed.");
    }

    function copyFailureReason(error) {
        switch (error?.name) {
            case "NotAllowedError":
                return "Clipboard permission was denied.";
            case "SecurityError":
                return "Clipboard access is blocked by the browser.";
            case "NotSupportedError":
                return "Clipboard copying is not supported by this browser.";
            default:
                return "The browser could not copy the text.";
        }
    }

    input.addEventListener("input", () => {
        setStatus(defaultStatus);
        updateControls();
    });

    unrollShortLinks.addEventListener("change", () => {
        try {
            window.localStorage.setItem(
                unrollPreferenceKey,
                String(unrollShortLinks.checked),
            );
        } catch (error) {
            console.warn("Could not save the unroll preference.", error);
        }
    });

    input.addEventListener("keydown", (event) => {
        if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
            event.preventDefault();
            form.requestSubmit();
        }
    });

    form.addEventListener("submit", async (event) => {
        event.preventDefault();

        if (openedFromFile) {
            setStatus(
                "Open this page through a web server, not directly from disk.",
                "error",
            );
            return;
        }

        if (busy || !hasText()) {
            return;
        }

        const original = input.value;
        const resolveShortLinks = unrollShortLinks.checked;
        const cleaningPromise = (async () => {
            const cleaner = await loadCleaner();
            setStatus("Removing trackers…", "busy");
            return resolveShortLinks
                ? await cleaner.wash(original)
                : await cleaner.wash_without_short_links(original);
        })();
        const clipboardWritePromise = startClipboardWrite(cleaningPromise);
        clipboardWritePromise?.catch((error) => {
            console.warn("Initial clipboard write failed.", error);
        });

        setBusy(true);
        setStatus("Preparing cleaner…", "busy");

        try {
            const cleaned = await cleaningPromise;
            input.value = cleaned;

            try {
                if (clipboardWritePromise) {
                    try {
                        await clipboardWritePromise;
                    } catch (error) {
                        console.warn("Clipboard write failed; trying the fallback.", error);
                        await copyText(cleaned);
                    }
                } else {
                    await copyText(cleaned);
                }
                setStatus(
                    cleaned === original
                        ? "Already clean."
                        : "Cleaned and copied to clipboard.",
                    "success",
                );
            } catch (error) {
                console.error("Clipboard copy failed.", error);
                setStatus(
                    `Cleaning finished, but copying failed. ${copyFailureReason(error)} Select the text and copy it manually.`,
                    "error",
                );
            }
        } catch (error) {
            console.error(error);
            setStatus("The cleaner could not run. Reload the page and try again.", "error");
        } finally {
            setBusy(false);
            input.focus();
        }
    });

    if (openedFromFile) {
        unrollShortLinks.disabled = true;
        setStatus(
            "Open this page through a web server, not directly from disk.",
            "error",
        );
    } else {
        setBusy(false);
    }
}
