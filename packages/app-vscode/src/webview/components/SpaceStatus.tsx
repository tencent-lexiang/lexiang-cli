import './SpaceStatus.css';

import React, { useCallback, useEffect, useState } from 'react';

import { useVscode } from '../hooks/useVscode';
import type { ExtensionMessage } from '../shared-types';

interface SpaceInfo {
  spaceId: string;
  spaceName: string;
  lastAccess: number;
  isActive: boolean;
  syncStats?: { total: number; synced: number };
}

interface StatusState {
  whoami?: { company_name: string; user_name: string };
  maxOpenSpaces: number;
  spaces: SpaceInfo[];
}

export const SpaceStatus: React.FC = () => {
  const [state, setState] = useState<StatusState>({ maxOpenSpaces: 5, spaces: [] });

  const handleMessage = useCallback((msg: ExtensionMessage) => {
    if ('data' in msg && (msg as Record<string, unknown>).type === 'update') {
      setState((msg as Record<string, unknown>).data as StatusState);
    }
  }, []);

  const { postMessage } = useVscode(handleMessage);

  useEffect(() => {
    postMessage({ type: 'ready' } as never);
  }, [postMessage]);

  const handleMaxChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const val = parseInt(e.target.value, 10);
    if (!isNaN(val) && val >= 0) {
      postMessage({ type: 'setMaxOpenSpaces', value: val } as never);
      setState(prev => ({ ...prev, maxOpenSpaces: val }));
    }
  };

  const handleCloseSpace = (spaceId: string) => {
    postMessage({ type: 'closeSpace', spaceId } as never);
  };

  const formatTime = (ts: number) => {
    if (!ts) return 'Never';
    const date = new Date(ts);
    return date.toLocaleString();
  };

  return (
    <div className="space-status-container">
      {state.whoami ? (
        <div className="whoami-section">
          <div className="whoami-info">
            <i className="codicon codicon-account"></i>
            <span>{state.whoami.company_name} - {state.whoami.user_name}</span>
          </div>
        </div>
      ) : (
        <div className="whoami-section unauthenticated">
          <div className="whoami-info">
            <i className="codicon codicon-sign-in"></i>
            <span>未登录</span>
          </div>
        </div>
      )}

      <div className="config-section">
        <label>Max Open Spaces:</label>
        <input 
          type="number" 
          value={state.maxOpenSpaces} 
          onChange={handleMaxChange} 
          min="0"
          className="vscode-input"
        />
        <div className="hint">0 = Unlimited</div>
      </div>

      <div className="spaces-list">
        <h3>知识库 ({state.spaces.length})</h3>
        {state.spaces.length === 0 ? (
          <div className="empty-state">
            <i className="codicon codicon-database"></i>
            <p className="empty-title">尚未添加知识库</p>
            <p className="empty-hint">选择或同步一个知识库开始使用</p>
          </div>
        ) : (
          <ul>
            {state.spaces.map(space => (
              <li key={space.spaceId} className="space-item">
                <div className="space-info">
                  <span className="space-name">{space.spaceName}</span>
                  <span className="space-time">{formatTime(space.lastAccess)}</span>
                  {space.syncStats && (
                    <span className="space-sync-stats" title="已同步内容节点数 / 总节点数">
                      <i className="codicon codicon-sync"></i> {space.syncStats.synced}/{space.syncStats.total}
                    </span>
                  )}
                </div>
                <button 
                  className="close-btn"
                  onClick={() => handleCloseSpace(space.spaceId)}
                  title="Close Space"
                >
                  <i className="codicon codicon-close"></i>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
};
