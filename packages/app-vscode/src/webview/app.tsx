import React, { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import { SpacePicker } from './components/SpacePicker.js';
import { SpaceStatus } from './components/SpaceStatus.js';

declare global {
  interface Window {
    viewType?: 'picker' | 'status';
  }
}

const container = document.getElementById('root');
const viewType = window.viewType || 'picker';

const Component = viewType === 'status' ? SpaceStatus : SpacePicker;

if (container) {
  createRoot(container).render(
    <StrictMode>
      <Component />
    </StrictMode>,
  );
}
