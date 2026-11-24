import { useEffect, useState } from 'react';
import type { Result } from '../shared/model';

export const phaseLabels = { focus: '专注', short_break: '短休息', long_break: '长休息' };

export function unwrap<T>(result: Result<T>) {
  if (!result.ok) throw new Error(result.message);
  return result.value;
}

export function useTimerNow(serverNow: number) {
  const [now, setNow] = useState(serverNow);
  useEffect(() => {
    const anchor = performance.now();
    setNow(serverNow);
    const id = window.setInterval(() => setNow(serverNow + performance.now() - anchor), 250);
    return () => window.clearInterval(id);
  }, [serverNow]);
  return now;
}

export function formatCountdown(endsAt: number, now: number) {
  const seconds = Math.max(0, Math.ceil((endsAt - now) / 1000));
  return `${String(Math.floor(seconds / 60)).padStart(2, '0')}:${String(seconds % 60).padStart(2, '0')}`;
}

export function playChime() {
  const context = new AudioContext();
  const start = context.currentTime;
  for (const [frequency, delay] of [[660, 0], [880, .16]]) {
    const oscillator = context.createOscillator();
    const volume = context.createGain();
    oscillator.frequency.value = frequency;
    volume.gain.setValueAtTime(.001, start + delay);
    volume.gain.exponentialRampToValueAtTime(.12, start + delay + .04);
    volume.gain.exponentialRampToValueAtTime(.001, start + delay + .45);
    oscillator.connect(volume).connect(context.destination);
    oscillator.start(start + delay);
    oscillator.stop(start + delay + .46);
  }
  window.setTimeout(() => void context.close(), 900);
}
