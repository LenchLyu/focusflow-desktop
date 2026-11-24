import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { tauriApi } from './tauri-api';

window.focusflow = tauriApi;
const label = getCurrentWebviewWindow().label;
if (label === 'launcher') {
  void import('../renderer/launcher');
} else if (label === 'mini') {
  void import('../renderer/mini');
} else {
  void import('../renderer/index');
}
