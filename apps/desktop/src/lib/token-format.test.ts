import { describe, expect, it } from "vitest";
import {
  formatFullTokenCount,
  formatCompactTokenCount,
} from "./token-format";

describe("token formatting", () => {
  it("groups full token counts with ASCII commas from one thousand", () => {
    expect(formatFullTokenCount(999)).toBe("999");
    expect(formatFullTokenCount(1_000)).toBe("1,000");
    expect(formatFullTokenCount(1_234)).toBe("1,234");
    expect(formatFullTokenCount(1_234_567)).toBe("1,234,567");
    expect(formatFullTokenCount(1_234_567_890)).toBe("1,234,567,890");
  });

  it("uses uppercase K, M, B and T units", () => {
    expect(formatCompactTokenCount(999)).toBe("999");
    expect(formatCompactTokenCount(1_000)).toBe("1K");
    expect(formatCompactTokenCount(10_200)).toBe("10.2K");
    expect(formatCompactTokenCount(3_500_000)).toBe("3.5M");
    expect(formatCompactTokenCount(1_200_000_000)).toBe("1.2B");
    expect(formatCompactTokenCount(10_200_000_000_000)).toBe("10.2T");
    expect(formatCompactTokenCount(999_950)).toBe("1M");
  });

  it("does not emit locale-specific separators or trailing zero decimals", () => {
    expect(formatCompactTokenCount(1_000_000)).toBe("1M");
    expect(formatCompactTokenCount(1_000_000_000)).toBe("1B");
    expect(formatCompactTokenCount(1_000_000_000_000)).toBe("1T");
  });
});
