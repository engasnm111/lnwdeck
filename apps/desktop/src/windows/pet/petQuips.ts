/**
 * Random speech for the desktop pet, localized.
 *
 * Quips mix real numbers from the quota dashboard (today's tokens, the
 * lowest remaining quota, plan names) with short personality lines, so every
 * tap surfaces something live. Plain text only — the bubble is decorative
 * and duplicates nothing sensitive. Unknown languages fall back to English.
 */

export interface QuipData {
  todayTokens: number;
  costUsd: number;
  currencySymbol: string;
  /** Lowest remaining percentage across published windows, if any. */
  lowestRemainingPercent: number | null;
  plan: string | null;
}

function formatCompact(n: number): string {
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(Math.round(n));
}

function pick<T>(items: readonly T[]): T {
  return items[Math.floor(Math.random() * items.length)];
}

/** Quip lines per language; the first two slots are token/cost/quota lines. */
const QUIPS: Record<string, { usedToday: string; costToday: string; quotaLeft: string; plan: string; idle: string[] }> = {
  en: {
    usedToday: "Used {tokens} tokens today",
    costToday: "{tokens} tokens, {symbol}{cost} today",
    quotaLeft: "{percent}% of a quota window left",
    plan: "On the {plan} plan",
    idle: ["Hello!", "Still watching your tokens...", "I could walk here all day", "Hover me anytime", "Click me again!", "Right-click for options"],
  },
  th: {
    usedToday: "วันนี้ใช้ไป {tokens} โทเคน",
    costToday: "{tokens} โทเคน, {symbol}{cost} วันนี้",
    quotaLeft: "เหลือโควต้า {percent}%",
    plan: "แผน {plan}",
    idle: ["สวัสดี!", "ยังคอยดูโทเคนของคุณอยู่...", "ฉันเดินแบบนี้ได้ทั้งวัน", "ชี้เมาส์ที่ฉันได้ทุกเมื่อ", "คลิกฉันอีกสิ!", "คลิกขวาเพื่อตัวเลือก"],
  },
  zh: { usedToday: "今天已使用 {tokens} 个令牌", costToday: "{tokens} 个令牌，今天 {symbol}{cost}", quotaLeft: "配额窗口还剩 {percent}%", plan: "当前套餐：{plan}", idle: ["你好！", "还在盯着你的令牌...", "我可以整天散步", "随时悬停查看", "再点我一下！", "右键查看选项"] },
  ja: { usedToday: "今日 {tokens} トークン使用", costToday: "{tokens} トークン、今日 {symbol}{cost}", quotaLeft: "クォータ残り {percent}%", plan: "プラン: {plan}", idle: ["こんにちは！", "トークンを見守っています...", "一日中歩けます", "いつでもホバーしてね", "もう一度クリック！", "右クリックでオプション"] },
  ko: { usedToday: "오늘 {tokens} 토큰 사용", costToday: "{tokens} 토큰, 오늘 {symbol}{cost}", quotaLeft: "할당량 {percent}% 남음", plan: "{plan} 플랜", idle: ["안녕하세요!", "토큰을 계속 지켜보고 있어요...", "하루 종일 걸을 수 있어요", "언제든 마우스를 올려보세요", "다시 클릭해 주세요!", "마우스 오른쪽 클릭으로 옵션"] },
  de: { usedToday: "Heute {tokens} Token verbraucht", costToday: "{tokens} Token, heute {symbol}{cost}", quotaLeft: "Noch {percent}% eines Kontingents", plan: "Im {plan}-Plan", idle: ["Hallo!", "Ich behalte deine Token im Auge...", "Ich könnte hier den ganzen Tag laufen", "Fahr mich jederzeit mit der Maus an", "Klick mich nochmal!", "Rechtsklick für Optionen"] },
  fr: { usedToday: "{tokens} jetons utilisés aujourd'hui", costToday: "{tokens} jetons, {symbol}{cost} aujourd'hui", quotaLeft: "Encore {percent}% d'une fenêtre de quota", plan: "Forfait {plan}", idle: ["Bonjour !", "Je surveille toujours vos jetons...", "Je pourrais marcher ici toute la journée", "Survolez-moi quand vous voulez", "Recliquez-moi !", "Clic droit pour les options"] },
  es: { usedToday: "{tokens} tokens usados hoy", costToday: "{tokens} tokens, {symbol}{cost} hoy", quotaLeft: "Queda el {percent}% de una ventana de cuota", plan: "En el plan {plan}", idle: ["¡Hola!", "Sigo vigilando tus tokens...", "Podría caminar aquí todo el día", "Pásame el cursor cuando quieras", "¡Haz clic de nuevo!", "Clic derecho para opciones"] },
  ru: { usedToday: "Сегодня использовано {tokens} токенов", costToday: "{tokens} токенов, {symbol}{cost} сегодня", quotaLeft: "Осталось {percent}% квоты", plan: "Тариф {plan}", idle: ["Привет!", "Продолжаю следить за токенами...", "Могу гулять здесь весь день", "Наведите курсор в любое время", "Кликните ещё раз!", "Правый клик — параметры"] },
};

function fill(template: string, vars: Record<string, string>): string {
  let text = template;
  for (const [name, value] of Object.entries(vars)) {
    text = text.replace(`{${name}}`, value);
  }
  return text;
}

export function pickPetQuip(data: QuipData, language = "en"): string {
  const quips = QUIPS[language] ?? QUIPS.en;
  const lines: string[] = [];
  if (data.todayTokens > 0) {
    lines.push(fill(quips.usedToday, { tokens: formatCompact(data.todayTokens) }));
    if (data.costUsd > 0) {
      lines.push(
        fill(quips.costToday, {
          tokens: formatCompact(data.todayTokens),
          symbol: data.currencySymbol,
          cost: data.costUsd.toFixed(2),
        }),
      );
    }
  }
  if (data.lowestRemainingPercent !== null) {
    lines.push(
      fill(quips.quotaLeft, { percent: String(Math.round(data.lowestRemainingPercent)) }),
    );
  }
  if (data.plan) {
    lines.push(fill(quips.plan, { plan: data.plan }));
  }
  lines.push(...quips.idle);
  return pick(lines);
}
