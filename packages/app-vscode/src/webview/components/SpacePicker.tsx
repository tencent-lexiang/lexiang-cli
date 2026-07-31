import './SpacePicker.css';

import React, { useCallback, useEffect, useRef,useState } from 'react';

import { useVscode } from '../hooks/useVscode.js';
import type { ExtensionMessage, SearchDocItem, SearchTarget, SpaceItem, TeamItem } from '../shared-types.js';

type Tab = 'recent' | 'team';

declare global {
  interface Window {
    __LEFS_INITIAL_SEARCH_TARGET__?: SearchTarget;
  }
}

function getInitialSearchTarget(): SearchTarget {
  return window.__LEFS_INITIAL_SEARCH_TARGET__ === 'entry' ? 'entry' : 'space';
}

export function SpacePicker(): React.ReactElement {
  const [tab, setTab] = useState<Tab>('recent');
  const [searchQuery, setSearchQuery] = useState('');
  const [searchTarget, setSearchTarget] = useState<SearchTarget>(getInitialSearchTarget);

  // recent spaces
  const [recentSpaces, setRecentSpaces] = useState<SpaceItem[]>([]);
  const [recentLoading, setRecentLoading] = useState(false);

  // teams
  const [teams, setTeams] = useState<TeamItem[]>([]);
  const [teamsLoading, setTeamsLoading] = useState(false);
  const [selectedTeam, setSelectedTeam] = useState<TeamItem | null>(null);

  // team spaces
  const [teamSpaces, setTeamSpaces] = useState<SpaceItem[]>([]);
  const [teamSpacesLoading, setTeamSpacesLoading] = useState(false);

  // search
  const [spaceSearchResults, setSpaceSearchResults] = useState<SpaceItem[]>([]);
  const [entrySearchResults, setEntrySearchResults] = useState<SearchDocItem[]>([]);
  const [searchLoading, setSearchLoading] = useState(false);

  const searchTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleMessage = useCallback((msg: ExtensionMessage) => {
    switch (msg.type) {
      case 'recentSpaces':
        setRecentSpaces(msg.spaces);
        setRecentLoading(msg.loading);
        break;
      case 'frequentTeams':
        setTeams((prev) => mergeTeams(msg.teams, prev));
        setTeamsLoading(msg.loading);
        break;
      case 'moreTeams':
        setTeams((prev) => mergeTeams(prev, msg.teams));
        setTeamsLoading(msg.loading);
        break;
      case 'teamSpaces':
        if (msg.teamId) {
          setTeamSpaces(msg.spaces);
          setTeamSpacesLoading(msg.loading);
        }
        break;
      case 'spaceSearchResults':
        setSpaceSearchResults(msg.spaces);
        setSearchLoading(msg.loading);
        break;
      case 'entrySearchResults':
        setEntrySearchResults(msg.docs);
        setSearchLoading(msg.loading);
        break;
      case 'error':
        console.error('[SpacePicker]', msg.message);
        break;
    }
  }, []);

  const { postMessage } = useVscode(handleMessage);

  useEffect(() => {
    postMessage({ type: 'ready' });
    setRecentLoading(true);
    postMessage({ type: 'loadRecentSpaces' });
  }, [postMessage]);

  useEffect(() => {
    if (tab === 'team' && teams.length === 0) {
      setTeamsLoading(true);
      postMessage({ type: 'loadFrequentTeams' });
      postMessage({ type: 'loadMoreTeams' });
    }
  }, [tab, teams.length, postMessage]);

  useEffect(() => {
    if (selectedTeam) {
      setTeamSpacesLoading(true);
      setTeamSpaces([]);
      postMessage({ type: 'loadTeamSpaces', teamId: selectedTeam.id });
    }
  }, [selectedTeam, postMessage]);

  useEffect(() => {
    if (searchTimerRef.current) {
      clearTimeout(searchTimerRef.current);
    }

    if (searchQuery.trim().length === 0) {
      setSpaceSearchResults([]);
      setEntrySearchResults([]);
      setSearchLoading(false);
      return;
    }

    setSearchLoading(true);
    searchTimerRef.current = setTimeout(() => {
      postMessage({ type: 'search', keyword: searchQuery.trim(), target: searchTarget });
    }, 300);

    return () => {
      if (searchTimerRef.current) clearTimeout(searchTimerRef.current);
    };
  }, [searchQuery, searchTarget, postMessage]);

  const handleSelectSpace = useCallback((space: SpaceItem) => {
    postMessage({ type: 'selectSpace', space });
  }, [postMessage]);

  const handleSelectEntry = useCallback((doc: SearchDocItem) => {
    postMessage({ type: 'selectEntry', doc });
  }, [postMessage]);

  const handleCancel = useCallback(() => {
    postMessage({ type: 'cancel' });
  }, [postMessage]);

  const isSearching = searchQuery.trim().length > 0;

  return (
    <div className="space-picker">
      <div className="picker-header">
        <div className="tabs">
          <button
            className={`tab ${tab === 'recent' ? 'active' : ''}`}
            onClick={() => setTab('recent')}
          >
            最近知识库
          </button>
          <button
            className={`tab ${tab === 'team' ? 'active' : ''}`}
            onClick={() => setTab('team')}
          >
            团队知识库
          </button>
        </div>

        <div className="search-box">
          <span className="search-icon codicon codicon-search" />
          <input
            type="text"
            placeholder={searchTarget === 'space' ? '搜索知识库' : '搜索知识'}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
          />
          <select
            className="search-target-select"
            value={searchTarget}
            onChange={(e) => setSearchTarget(e.target.value as SearchTarget)}
          >
            <option value="space">知识库</option>
            <option value="entry">知识</option>
          </select>
        </div>
      </div>

      <div className="picker-content">
        {isSearching ? (
          searchTarget === 'space' ? (
            <SearchSpaceView
              results={spaceSearchResults}
              loading={searchLoading}
              onSelect={handleSelectSpace}
            />
          ) : (
            <SearchEntryView
              results={entrySearchResults}
              loading={searchLoading}
              onSelect={handleSelectEntry}
            />
          )
        ) : tab === 'recent' ? (
          <RecentView
            spaces={recentSpaces}
            loading={recentLoading}
            onSelect={handleSelectSpace}
          />
        ) : (
          <TeamView
            teams={teams}
            teamsLoading={teamsLoading}
            selectedTeam={selectedTeam}
            onSelectTeam={setSelectedTeam}
            spaces={teamSpaces}
            spacesLoading={teamSpacesLoading}
            onSelectSpace={handleSelectSpace}
            onLoadMore={() => postMessage({ type: 'loadMoreTeams' })}
          />
        )}
      </div>

      <div className="picker-footer">
        <button className="btn btn-default" onClick={handleCancel}>取消</button>
        <button className="btn btn-primary" disabled>确定</button>
      </div>
    </div>
  );
}

function RecentView({
  spaces,
  loading,
  onSelect,
}: {
  spaces: SpaceItem[];
  loading: boolean;
  onSelect: (s: SpaceItem) => void;
}): React.ReactElement {
  if (loading && spaces.length === 0) {
    return <div className="loading">加载中...</div>;
  }
  if (spaces.length === 0) {
    return <div className="empty">暂无最近访问的知识库</div>;
  }
  return (
    <div className="space-list">
      {spaces.map((space) => (
        <div key={space.id} className="space-item" onClick={() => onSelect(space)}>
          <span className="space-icon codicon codicon-book" />
          <span className="space-name">{space.name}</span>
        </div>
      ))}
    </div>
  );
}

function TeamView({
  teams,
  teamsLoading,
  selectedTeam,
  onSelectTeam,
  spaces,
  spacesLoading,
  onSelectSpace,
  onLoadMore,
}: {
  teams: TeamItem[];
  teamsLoading: boolean;
  selectedTeam: TeamItem | null;
  onSelectTeam: (t: TeamItem) => void;
  spaces: SpaceItem[];
  spacesLoading: boolean;
  onSelectSpace: (s: SpaceItem) => void;
  onLoadMore: () => void;
}): React.ReactElement {
  return (
    <div className="team-view">
      <div className="team-list-panel">
        <div className="panel-header">
          <span>团队</span>
          <button className="icon-btn codicon codicon-add" title="加载更多团队" onClick={onLoadMore} />
        </div>
        <div className="team-list">
          {teamsLoading && teams.length === 0 ? (
            <div className="loading">加载中...</div>
          ) : teams.length === 0 ? (
            <div className="empty">暂无团队</div>
          ) : (
            teams.map((team) => (
              <div
                key={team.id}
                className={`team-item ${selectedTeam?.id === team.id ? 'active' : ''}`}
                onClick={() => onSelectTeam(team)}
              >
                <span className="team-icon codicon codicon-organization" />
                <span className="team-name" title={team.name}>{team.name}</span>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="space-list-panel">
        {selectedTeam ? (
          <>
            <div className="panel-header">
              <span>"{selectedTeam.name}"的知识库</span>
            </div>
            <div className="space-list">
              {spacesLoading && spaces.length === 0 ? (
                <div className="loading">加载中...</div>
              ) : spaces.length === 0 ? (
                <div className="empty">该团队下暂无知识库</div>
              ) : (
                spaces.map((space) => (
                  <div key={space.id} className="space-item" onClick={() => onSelectSpace(space)}>
                    <span className="space-icon codicon codicon-book" />
                    <span className="space-name">{space.name}</span>
                  </div>
                ))
              )}
            </div>
          </>
        ) : (
          <div className="empty-hint">请先选择左侧的团队</div>
        )}
      </div>
    </div>
  );
}

function SearchSpaceView({
  results,
  loading,
  onSelect,
}: {
  results: SpaceItem[];
  loading: boolean;
  onSelect: (s: SpaceItem) => void;
}): React.ReactElement {
  if (loading && results.length === 0) {
    return <div className="loading">搜索中...</div>;
  }
  if (results.length === 0) {
    return <div className="empty">未找到匹配的知识库</div>;
  }
  return (
    <div className="space-list">
      {results.map((space) => (
        <div key={space.id} className="space-item" onClick={() => onSelect(space)}>
          <span className="space-icon codicon codicon-book" />
          <span className="space-name">{renderHighlightedText(space.name)}</span>
        </div>
      ))}
    </div>
  );
}

function SearchEntryView({
  results,
  loading,
  onSelect,
}: {
  results: SearchDocItem[];
  loading: boolean;
  onSelect: (doc: SearchDocItem) => void;
}): React.ReactElement {
  if (loading && results.length === 0) {
    return <div className="loading">搜索中...</div>;
  }
  if (results.length === 0) {
    return <div className="empty">未找到匹配的知识</div>;
  }

  return (
    <div className="space-list">
      {results.map((doc) => (
        <div key={`${doc.spaceId}:${doc.entryId}`} className="space-item" onClick={() => onSelect(doc)}>
          <span className="space-icon codicon codicon-file" />
          <div className="search-doc-main">
            <span className="space-name">{renderHighlightedText(doc.title)}</span>
            <span className="search-doc-meta">space: {doc.spaceName || doc.spaceId}</span>
          </div>
        </div>
      ))}
    </div>
  );
}

function renderHighlightedText(raw: string): React.ReactNode {
  if (!raw.includes('<em>') && !raw.includes('</em>')) {
    return raw;
  }

  const tokens = raw.split(/(<\/?em>)/gi);
  const nodes: React.ReactNode[] = [];
  let isHighlight = false;
  let index = 0;

  for (const token of tokens) {
    const lower = token.toLowerCase();
    if (lower === '<em>') {
      isHighlight = true;
      continue;
    }
    if (lower === '</em>') {
      isHighlight = false;
      continue;
    }
    if (!token) continue;

    if (isHighlight) {
      nodes.push(
        <mark key={`hl-${index++}`} className="search-highlight">{token}</mark>,
      );
    } else {
      nodes.push(
        <React.Fragment key={`txt-${index++}`}>{token}</React.Fragment>,
      );
    }
  }

  return <>{nodes}</>;
}

function mergeTeams(primary: TeamItem[], secondary: TeamItem[]): TeamItem[] {
  const seen = new Set(primary.map((t) => t.id));
  const result = [...primary];
  for (const t of secondary) {
    if (!seen.has(t.id)) {
      seen.add(t.id);
      result.push(t);
    }
  }
  return result;
}
