/**
 * Tool helpers
 */

/**
 * OpenClaw tool result format
 */
export interface ToolResult<T = unknown> {
  content: Array<{ type: 'text'; text: string }>;
  details: T;
}

/**
 * Format tool result for OpenClaw
 */
export function formatToolResult<T>(data: T): ToolResult<T> {
  return {
    content: [{ type: 'text' as const, text: JSON.stringify(data, null, 2) }],
    details: data,
  };
}

/**
 * Format error result
 */
export function formatErrorResult(error: unknown): ToolResult<{ success: false; error: string }> {
  const errorMsg = error instanceof Error ? error.message : String(error);
  return formatToolResult({ success: false, error: errorMsg });
}
