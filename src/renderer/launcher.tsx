import { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { AlertCircle, GripVertical, LayoutGrid, Timer, X } from 'lucide-react';
import type { LauncherAction } from '../shared/model';
import { playChime, unwrap } from './runtime';
import { Button, IconButton } from './ui';
import './styles.css';

const api = window.focusflow;
const launcher = api.launcher!;

function Launcher() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const working = useRef(false);

  useEffect(() => {
    let mounted = true;
    async function syncTheme() {
      const result = await launcher.theme();
      if (mounted && result.ok) document.documentElement.dataset.theme = result.value;
    }
    const refreshOnShow = () => { if (document.visibilityState === 'visible') void syncTheme(); };
    void syncTheme();
    window.addEventListener('focus', refreshOnShow);
    document.addEventListener('visibilitychange', refreshOnShow);
    const removeChime = api.onChime(playChime);
    return () => {
      mounted = false;
      window.removeEventListener('focus', refreshOnShow);
      document.removeEventListener('visibilitychange', refreshOnShow);
      removeChime();
    };
  }, []);

  async function choose(action: LauncherAction) {
    if (working.current) return;
    working.current = true;
    setBusy(true);
    setError('');
    try { unwrap(await launcher.act(action)); }
    catch (error) { setError(error instanceof Error ? error.message : '无法切换窗口，请重试。'); }
    finally { working.current = false; setBusy(false); }
  }

  return <main className="launcher-bar" role="toolbar" aria-label="FocusFlow 悬浮启动器">
    <span className={`launcher-grip${error ? ' launcher-grip-error' : ''}`} data-tauri-drag-region
      title={error || '按住拖动启动器'} aria-hidden>
      {error ? <AlertCircle size={15} /> : <GripVertical size={17} />}
    </span>
    <Button icon={LayoutGrid} variant="ghost" aria-label="打开主窗口" disabled={busy} onClick={() => void choose('openMain')}>主窗口</Button>
    <Button icon={Timer} variant="ghost" aria-label="打开番茄小窗" disabled={busy} onClick={() => void choose('openMini')}>番茄</Button>
    <IconButton icon={X} label="关闭启动器" disabled={busy} onClick={() => void choose('dismiss')} />
    {error && <p className="launcher-error" role="alert">{error}</p>}
  </main>;
}

createRoot(document.getElementById('root')!).render(<Launcher />);
