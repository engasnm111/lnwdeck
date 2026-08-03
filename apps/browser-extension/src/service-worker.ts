const NATIVE_HOST_NAME = "app.lnwdeck.browser_helper";

let port: chrome.runtime.Port | null = null;

function connectNative(): chrome.runtime.Port {
  if (!port) {
    port = chrome.runtime.connectNative(NATIVE_HOST_NAME);
    port.onDisconnect.addListener(() => {
      port = null;
    });
  }
  return port;
}

function sendQuotaUpdate(provider: string, remaining: number | null): void {
  const msg = {
    type: "quota_update",
    version: 1,
    timestamp: new Date().toISOString(),
    nonce: crypto.randomUUID(),
    payload: {
      provider,
      remaining,
    },
  };

  try {
    const p = connectNative();
    p.postMessage(msg);
  } catch {
    // Native host not available — expected on first install
  }
}

chrome.runtime.onMessage.addListener(
  (message: { type: string; provider: string; remaining: number | null }) => {
    if (message.type === "quota_detected") {
      sendQuotaUpdate(message.provider, message.remaining);
    }
  },
);
