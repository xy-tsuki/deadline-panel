import { CSSProperties, useLayoutEffect, useRef, useState } from "react";

// Geometry is measured after font/language/layout changes; no per-frame IPC.
export function LiquidTextSegments<T extends string>({ value, options, label, onChange }: {
  value: T; options: { value: T; label: string }[]; label: string;
  onChange: (value: T) => Promise<void>;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [draft, setDraft] = useState<T | null>(null);
  const [rect, setRect] = useState({ left: 4, width: 54 });
  const dragging = useRef(false);
  const pending = useRef(value);
  const selected = draft ?? value;
  useLayoutEffect(() => {
    const node = root.current;
    if (!node) return;
    const measure = () => {
      const button = node.querySelector<HTMLButtonElement>('button[aria-checked="true"]');
      if (button) setRect({ left: button.offsetLeft, width: button.offsetWidth });
    };
    measure(); const observer = new ResizeObserver(measure); observer.observe(node);
    return () => observer.disconnect();
  }, [selected, options.map(option => option.label).join("|")]);
  const commit = async (next: T) => { try { await onChange(next); } finally { setDraft(null); } };
  const choose = (clientX: number) => {
    const node = root.current!;
    // getBoundingClientRect 含祖先的 transform 缩放（设置页会整体缩小），offset* 不含，先换算回未缩放坐标
    const bounds = node.getBoundingClientRect();
    const scale = node.offsetWidth > 0 ? bounds.width / node.offsetWidth : 1;
    const x = (clientX - bounds.left) / (scale || 1);
    const buttons = [...node.querySelectorAll("button")];
    const index = buttons.findIndex(button => x < button.offsetLeft + button.offsetWidth);
    pending.current = options[index < 0 ? options.length - 1 : index].value;
    setDraft(pending.current);
  };
  return <div ref={root} className="liquid-text-segments" role="radiogroup" aria-label={label}
    style={{ "--selection-left": `${rect.left}px`, "--selection-width": `${rect.width}px` } as CSSProperties}
    onPointerDown={event => { if (event.button !== 0) return; event.currentTarget.dataset.input = "pointer"; event.preventDefault(); dragging.current = true; event.currentTarget.setPointerCapture(event.pointerId); choose(event.clientX); if (event.target instanceof HTMLButtonElement) event.target.focus(); }}
    onPointerMove={event => { if (dragging.current) choose(event.clientX); }}
    onPointerUp={event => { if (!dragging.current) return; choose(event.clientX); dragging.current = false; event.currentTarget.releasePointerCapture(event.pointerId); void commit(pending.current); }}
    onPointerCancel={() => { dragging.current = false; setDraft(null); }}>
    <span className="liquid-text-segments__selection" aria-hidden="true" />
    {options.map((option, index) => <button type="button" key={option.value} role="radio" aria-checked={selected === option.value} tabIndex={selected === option.value ? 0 : -1}
      onClick={event => { if (event.detail === 0) void commit(option.value); }}
      onKeyDown={event => {
        event.currentTarget.parentElement!.dataset.input = "keyboard";
        let next = index;
        if (["ArrowRight", "ArrowDown"].includes(event.key)) next = (index + 1) % options.length;
        else if (["ArrowLeft", "ArrowUp"].includes(event.key)) next = (index + options.length - 1) % options.length;
        else if (event.key === "Home") next = 0;
        else if (event.key === "End") next = options.length - 1;
        else return;
        event.preventDefault(); root.current?.querySelectorAll("button")[next].focus(); setDraft(options[next].value); void commit(options[next].value);
      }}>{option.label}</button>)}
  </div>;
}
