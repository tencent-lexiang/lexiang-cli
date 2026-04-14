import { describe, expect, test } from 'vitest';

import { parseCompanyFromFromUrl, shouldPromptCompanyFrom } from './company-from.js';

describe('parseCompanyFromFromUrl', () => {
  test('should parse company_from from mine url', () => {
    const result = parseCompanyFromFromUrl('https://csig.lexiangla.com/mine?company_from=csig');
    expect(result).toBe('csig');
  });

  test('should parse company_from from pages url', () => {
    const result = parseCompanyFromFromUrl('https://csig.lexiangla.com/pages/18700e274557428483226c8a008984c4?company_from=csig');
    expect(result).toBe('csig');
  });

  test('should parse company_from from spaces url', () => {
    const result = parseCompanyFromFromUrl('https://csig.lexiangla.com/spaces/91b1911c3e024af5a19baf9a5c134c07/ai?company_from=csig');
    expect(result).toBe('csig');
  });

  test('should return null when company_from is missing', () => {
    const result = parseCompanyFromFromUrl('https://csig.lexiangla.com/mine');
    expect(result).toBeNull();
  });

  test('should return null for invalid url', () => {
    const result = parseCompanyFromFromUrl('not-a-url');
    expect(result).toBeNull();
  });
});

describe('shouldPromptCompanyFrom', () => {
  test('should skip prompt when mcp auth exists', () => {
    const result = shouldPromptCompanyFrom({
      hasStoredCompanyFrom: false,
      hasLegacyCompanyFrom: false,
      hasMcpAuthInfo: true,
      forcePrompt: false,
    });
    expect(result).toBe(false);
  });

  test('should prompt when no company and no auth info', () => {
    const result = shouldPromptCompanyFrom({
      hasStoredCompanyFrom: false,
      hasLegacyCompanyFrom: false,
      hasMcpAuthInfo: false,
      forcePrompt: false,
    });
    expect(result).toBe(true);
  });

  test('should always prompt on forcePrompt', () => {
    const result = shouldPromptCompanyFrom({
      hasStoredCompanyFrom: true,
      hasLegacyCompanyFrom: true,
      hasMcpAuthInfo: true,
      forcePrompt: true,
    });
    expect(result).toBe(true);
  });
});
