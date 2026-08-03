import React from 'react';
import ReactDOM from 'react-dom/client';
import { getCurrentWindow } from '@tauri-apps/api/window';
import MascotButton from './components/MascotButton';
import PanelWindow from './components/PanelWindow';
import './i18n/index';
import './index.css';

const windowLabel = getCurrentWindow().label;

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    {windowLabel === 'panel' ? <PanelWindow /> : <MascotButton />}
  </React.StrictMode>,
);
