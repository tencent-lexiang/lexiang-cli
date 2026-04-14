/** Webview ↔ Extension 消息协议 */

export interface TeamItem {
  id: string;
  name: string;
}

export interface SpaceItem {
  id: string;
  name: string;
  teamId?: string;
}

export interface SearchDocItem {
  entryId: string;
  title: string;
  spaceId: string;
  spaceName?: string;
  teamId?: string;
  targetType?: string;
}

export type SearchTarget = 'space' | 'entry';

export type PickerSelection =
  | { kind: 'space'; space: SpaceItem }
  | { kind: 'entry'; doc: SearchDocItem };

// ── Webview → Extension ──

export type WebviewMessage =
  | { type: 'ready' }
  | { type: 'loadRecentSpaces' }
  | { type: 'loadFrequentTeams' }
  | { type: 'loadMoreTeams' }
  | { type: 'loadTeamSpaces'; teamId: string }
  | { type: 'search'; keyword: string; target: SearchTarget }
  | { type: 'selectSpace'; space: SpaceItem }
  | { type: 'selectEntry'; doc: SearchDocItem }
  | { type: 'cancel' };

// ── Extension → Webview ──

export type ExtensionMessage =
  | { type: 'recentSpaces'; spaces: SpaceItem[]; loading: boolean }
  | { type: 'frequentTeams'; teams: TeamItem[]; loading: boolean }
  | { type: 'moreTeams'; teams: TeamItem[]; loading: boolean }
  | { type: 'teamSpaces'; teamId: string; spaces: SpaceItem[]; loading: boolean }
  | { type: 'spaceSearchResults'; spaces: SpaceItem[]; loading: boolean }
  | { type: 'entrySearchResults'; docs: SearchDocItem[]; loading: boolean }
  | { type: 'error'; message: string };
