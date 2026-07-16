"use client";

import { useId, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { fixtureApi } from "@/lib/api/client";
import { parseGithubTarget } from "@/lib/state/github-url";

// Submission form and live canonical preview (specification 12.5). Host
// validation runs client-side to guide the user; the server repeats it.

export function SubmissionForm() {
  const router = useRouter();
  const inputId = useId();
  const previewId = useId();
  const errorId = useId();
  const [value, setValue] = useState("");
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  const preview = useMemo(() => {
    if (value.trim() === "") return null;
    return parseGithubTarget(value);
  }, [value]);

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setSubmitError(null);
    const outcome = await fixtureApi.submit(value);
    if (outcome.kind === "invalid") {
      setSubmitError(outcome.error);
      setPending(false);
      return;
    }
    router.push(`/evaluations/${outcome.id}`);
  }

  const describedBy = [
    preview ? previewId : null,
    submitError ? errorId : null,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <form className="submit-form" onSubmit={onSubmit} noValidate>
      <label htmlFor={inputId}>GitHub repository URL</label>
      <input
        id={inputId}
        name="repository"
        type="text"
        inputMode="url"
        autoComplete="off"
        placeholder="https://github.com/owner/repository"
        value={value}
        onChange={(event) => setValue(event.target.value)}
        aria-describedby={describedBy || undefined}
        aria-invalid={submitError !== null || preview?.ok === false}
      />

      <div id={previewId} aria-live="polite">
        {preview?.ok && (
          <p className="muted">
            Canonical repository:{" "}
            <strong>
              {preview.source.namespace}/{preview.source.repository}
            </strong>{" "}
            on {preview.source.provider}
          </p>
        )}
        {preview && !preview.ok && (
          <p className="field-error">{preview.error}</p>
        )}
      </div>

      {submitError && (
        <p id={errorId} className="field-error" role="alert">
          {submitError}
        </p>
      )}

      <button type="submit" disabled={pending}>
        {pending ? "Submitting…" : "Analyze repository"}
      </button>
    </form>
  );
}
