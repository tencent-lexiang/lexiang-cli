/** 用于 QuickPick 的知识库选项 */
export interface SpaceQuickPickItem {
  label: string;
  description?: string;
  detail?: string;
  spaceId: string;
  spaceName: string;
}
