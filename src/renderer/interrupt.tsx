import { useState, type FormEvent } from 'react';
import type { ActiveSession } from '../shared/model';
import { Button, Modal, type Run } from './ui';

const reasons = ['临时有事', '接到电话或消息', '需要处理紧急任务', '身体不适或需要休息', '其他'];

export function Interrupt({ session, run, onClose, compact = false, error }: {
  session: ActiveSession; run: Run; onClose: () => void; compact?: boolean; error?: string;
}) {
  const [selected, setSelected] = useState('');
  const [custom, setCustom] = useState('');
  const [busy, setBusy] = useState(false);
  const hasPreset = Boolean(selected && selected !== '其他');
  const customLabel = hasPreset ? '补充说明（可选）' : '填写原因';
  const reason = hasPreset ? [selected, custom.trim()].filter(Boolean).join(' · ') : custom.trim();

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!reason || busy) return;
    setBusy(true);
    try {
      if (await run({ type: 'interrupt', id: session.id, reason })) onClose();
    } finally {
      setBusy(false);
    }
  }

  return <Modal title="打断番茄" onClose={onClose} className={compact ? 'mini-interrupt' : undefined}><form className="form" onSubmit={submit}>
    {compact ? <select aria-label="打断原因" value={selected} onChange={(event) => setSelected(event.target.value)}>
      <option value="">选择打断原因</option>
      {reasons.map((item) => <option key={item}>{item}</option>)}
    </select> : <>
      <p className="muted">第 {session.current_cycle} 轮将记为已打断。已完成的番茄和记录的想法会保留。</p>
      <fieldset className="reasons"><legend>打断原因</legend>{reasons.map((item) => <label key={item}>
        <input type="radio" name="reason" value={item} checked={selected === item} onChange={() => setSelected(item)} />{item}
      </label>)}</fieldset>
    </>}
    <label>{!compact && customLabel}<textarea rows={compact ? 1 : 3} maxLength={450}
      aria-label={compact ? customLabel : undefined} placeholder={compact ? customLabel : undefined}
      value={custom} onChange={(event) => setCustom(event.target.value)} /></label>
    {error && <p className="mini-error" role="alert">{error}</p>}
    <footer><Button onClick={onClose}>返回专注</Button><Button variant="danger" type="submit" disabled={!reason || busy}>确认打断</Button></footer>
  </form></Modal>;
}
