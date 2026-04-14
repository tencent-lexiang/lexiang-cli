import { useCallback, useEffect, useRef } from 'react';

import type { ExtensionMessage,WebviewMessage } from '../shared-types.js';

interface VsCodeApi {
  postMessage(message: WebviewMessage): void;
  getState(): unknown;
  setState(state: unknown): void;
}

declare function acquireVsCodeApi(): VsCodeApi;

let vscodeApi: VsCodeApi | null = null;

function getVscodeApi(): VsCodeApi {
  if (!vscodeApi) {
    vscodeApi = acquireVsCodeApi();
  }
  return vscodeApi;
}

export function useVscode(onMessage: (msg: ExtensionMessage) => void): {
  postMessage: (msg: WebviewMessage) => void;
} {
  const callbackRef = useRef(onMessage);
  callbackRef.current = onMessage;

  useEffect(() => {
    const handler = (event: MessageEvent<ExtensionMessage>): void => {
      callbackRef.current(event.data);
    };
    window.addEventListener('message', handler);
    return () => window.removeEventListener('message', handler);
  }, []);

  const postMessage = useCallback((msg: WebviewMessage) => {
    getVscodeApi().postMessage(msg);
  }, []);

  return { postMessage };
}
