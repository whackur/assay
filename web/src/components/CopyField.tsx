"use client";

import { useId, useState } from "react";

// A read-only value with a copy button. Success confirmation is quiet inline
// text (the effect of a clipboard write is otherwise invisible).

export function CopyField({ label, value }: { label: string; value: string }) {
  const inputId = useId();
  const [copied, setCopied] = useState(false);

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard unavailable (permissions, http). The field stays selectable.
    }
  }

  return (
    <div>
      <label className="visually-hidden" htmlFor={inputId}>
        {label}
      </label>
      <div className="copy-row">
        <input
          id={inputId}
          type="text"
          readOnly
          value={value}
          onFocus={(event) => event.target.select()}
        />
        <button type="button" className="quiet" onClick={copy}>
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
    </div>
  );
}
