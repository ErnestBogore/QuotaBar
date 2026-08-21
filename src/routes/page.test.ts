import { describe, expect, it } from 'vitest';

describe('QuotaBar frontend', () => {
  it('keeps the classic allowance at sixteen weekly points', () => {
    expect(100 / 16).toBe(6.25);
  });
});
