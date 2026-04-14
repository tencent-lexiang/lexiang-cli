export interface CompanyFromPromptOptions {
  hasStoredCompanyFrom: boolean;
  hasLegacyCompanyFrom: boolean;
  hasMcpAuthInfo: boolean;
  forcePrompt: boolean;
}

export function parseCompanyFromFromUrl(input: string): string | null {
  const normalized = input.trim();
  if (!normalized) return null;

  try {
    const url = new URL(normalized);
    const companyFrom = url.searchParams.get('company_from')?.trim();
    return companyFrom ? companyFrom : null;
  } catch {
    return null;
  }
}

export function shouldPromptCompanyFrom(options: CompanyFromPromptOptions): boolean {
  if (options.forcePrompt) return true;
  if (options.hasStoredCompanyFrom) return false;
  if (options.hasLegacyCompanyFrom) return false;
  if (options.hasMcpAuthInfo) return false;
  return true;
}
