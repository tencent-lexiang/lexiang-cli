import * as vscode from 'vscode';

import { WebDavManager } from './webdav-manager.js';

export interface SpaceAccessInfo {
    spaceId: string;
    lastAccess: number;
    spaceName: string;
}

export class SpaceManager {
    private accessMap = new Map<string, number>();
    private _onDidChange = new vscode.EventEmitter<void>();
    readonly onDidChange = this._onDidChange.event;
    
    constructor(private webdavManager: WebDavManager) {
        // 监听知识库变更，触发 LRU 检查
        this.webdavManager.onDidChange(() => {
             this._onDidChange.fire();
             this.checkLimit();
        });
    }
    
    public getRecentSpaces(): SpaceAccessInfo[] {
        const mounted = this.webdavManager.getAll();
        return mounted.map(s => ({
            spaceId: s.spaceId,
            spaceName: s.spaceName,
            lastAccess: this.accessMap.get(s.spaceId) || 0
        })).sort((a, b) => b.lastAccess - a.lastAccess);
    }
    
    public async closeSpace(spaceId: string) {
        await this.webdavManager.removeSpace(spaceId);
    }

    /** 清空内存中的访问时间戳 */
    public clear(): void {
        this.accessMap.clear();
        this._onDidChange.fire();
    }

    private async checkLimit() {
        const config = vscode.workspace.getConfiguration('lefs');
        const max = config.get<number>('maxOpenSpaces', 5);
        if (max <= 0) return; // 0 means unlimited
        
        const mounted = this.webdavManager.getAll();
        if (mounted.length <= max) return;
        
        // Sort by access time (oldest first)
        // If never accessed (undefined), treat as oldest (0)
        const sorted = mounted.map(s => ({
            ...s,
            lastAccess: this.accessMap.get(s.spaceId) || 0
        })).sort((a, b) => a.lastAccess - b.lastAccess);
        
        const toRemoveCount = mounted.length - max;
        // Close oldest spaces
        const toRemove = sorted.slice(0, toRemoveCount);
        
        for (const s of toRemove) {
            await this.webdavManager.removeSpace(s.spaceId);
        }
    }
}
