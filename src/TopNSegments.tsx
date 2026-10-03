import { CSSProperties, PointerEvent, useRef, useState } from "react";
import { FocusLimit } from "./store/deadlineStore";
const values: FocusLimit[] = [3, 5, 10];
export function TopNSegments({ value, label, onChange }: { value: FocusLimit; label: string; onChange: (value: FocusLimit) => Promise<void> }) {
  const [draft, setDraft] = useState<FocusLimit | null>(null);
  const dragging = useRef(false);
  const current = useRef(value);
  const selected = draft ?? value;
  const choose = (event: PointerEvent<HTMLDivElement>) => {
    const x = event.clientX - event.currentTarget.getBoundingClientRect().left - 4;
    const index = Math.max(0, Math.min(values.length - 1, Math.floor(x / 50)));
    current.current = values[index]; setDraft(values[index]);
  };
  const commit = async (next: FocusLimit) => { try { await onChange(next); } finally { setDraft(null); } };
  return <div className="topn-segments" role="radiogroup" aria-label={label} style={{ "--segment-index": values.indexOf(selected) } as CSSProperties}
    onPointerDown={(event) => {
      if (event.button !== 0) return;
      event.currentTarget.dataset.input = "pointer";
      event.preventDefault(); dragging.current = true; event.currentTarget.setPointerCapture(event.pointerId); choose(event);
      if (event.target instanceof HTMLButtonElement) event.target.focus();
    }}
    onPointerMove={(event) => { if (dragging.current) choose(event); }}
    onPointerUp={(event) => { if (!dragging.current) return; choose(event); dragging.current = false; event.currentTarget.releasePointerCapture(event.pointerId); void commit(current.current); }}
    onPointerCancel={() => { dragging.current = false; setDraft(null); }}>
    <span className="topn-segments__selection" aria-hidden="true" />
    {values.map((option, index) => <button key={option} type="button" role="radio" aria-checked={selected === option} tabIndex={selected === option ? 0 : -1}
      onClick={(event) => { if (event.detail === 0) void commit(option); }}
      onKeyDown={(event) => {
        event.currentTarget.parentElement!.dataset.input = "keyboard";
        let next = index;
        if (["ArrowRight", "ArrowDown"].includes(event.key)) next = (index + 1) % values.length;
        else if (["ArrowLeft", "ArrowUp"].includes(event.key)) next = (index + values.length - 1) % values.length;
        else if (event.key === "Home") next = 0;
        else if (event.key === "End") next = values.length - 1;
        else return;
        event.preventDefault(); (event.currentTarget.parentElement?.querySelectorAll("button")[next] as HTMLButtonElement)?.focus();
        setDraft(values[next]); void commit(values[next]);
      }}>Top {option}</button>)}
  </div>;
}
