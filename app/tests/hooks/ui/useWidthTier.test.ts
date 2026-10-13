import { describe, expect, test } from "bun:test";

import { widthTier } from "@/hooks/ui/useWidthTier";

const TIERS = { medium: 560, wide: 880 };

describe("widthTier", () => {
  test("each bound belongs to the tier above it", () => {
    expect(widthTier(320, TIERS)).toBe("narrow");
    expect(widthTier(559.5, TIERS)).toBe("narrow");
    expect(widthTier(560, TIERS)).toBe("medium");
    expect(widthTier(879, TIERS)).toBe("medium");
    expect(widthTier(880, TIERS)).toBe("wide");
    expect(widthTier(2000, TIERS)).toBe("wide");
  });
});
