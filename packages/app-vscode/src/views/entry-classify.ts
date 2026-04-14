/**
 * Entry classification — re-exported from lx-types.ts.
 *
 * The classifyMcpEntry function is now defined alongside the McpEntry type
 * it operates on, in rpc/lx-types.ts (aligned with Rust VFS).
 *
 * This file preserves backward compatibility for existing imports.
 */
export { classifyMcpEntry as classifyEntryRow } from '../rpc/lx-types.js';
