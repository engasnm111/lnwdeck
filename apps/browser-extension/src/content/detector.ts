function detectQuota(): { provider: string; remaining: number | null } | null {
    const url = window.location.hostname;

    if (url.includes("chatgpt.com") || url.includes("openai.com")) {
        const capsEl = document.querySelector("[data-testid='chatgpt-usage-bar']");
        if (capsEl) {
            return { provider: "openai", remaining: null };
        }
    }

    if (url.includes("claude.ai") || url.includes("anthropic.com")) {
        const usageText = document.body.innerText;
        if (usageText.includes("usage") || usageText.includes("remaining")) {
            return { provider: "anthropic", remaining: null };
        }
    }

    if (url.includes("gemini.google.com")) {
        return { provider: "google", remaining: null };
    }

    return null;
}

const result = detectQuota();
if (result) {
    chrome.runtime.sendMessage({
        type: "quota_detected",
        provider: result.provider,
        remaining: result.remaining,
    });
}
