"use client";

import { useId, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
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
    const parsed = parseGithubTarget(value);
    if (!parsed.ok) {
      setSubmitError(parsed.error);
      setPending(false);
      return;
    }
    let response: Response;
    try {
      response = await fetch("/api/project-evaluations", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ repository: parsed.canonicalUrl }),
      });
    } catch {
      setSubmitError("The hosted API is unavailable. Your repository was not scored or discarded.");
      setPending(false);
      return;
    }
    if (!response.ok) {
      const retryAfter = Number(response.headers.get("retry-after"));
      setSubmitError(response.status === 400
        ? "Enter a public GitHub owner/repository or canonical URL."
        : response.status === 429
          ? `Submission capacity is cooling down. Try again${Number.isFinite(retryAfter) && retryAfter > 0 ? ` in ${retryAfter} seconds` : " later"}.`
          : "The hosted API is unavailable. Try again after the service recovers.");
      setPending(false);
      return;
    }
    router.push(`/projects/github/${parsed.source.namespace}/${parsed.source.repository}`);
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
      <div className="submit-row">
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
        <button type="submit" disabled={pending}>
          {pending ? "Submitting…" : "Assay it"}
        </button>
      </div>

      <div id={previewId} aria-live="polite">
        {preview?.ok && (
          <p className="canonical-preview">
            canonical: <strong>
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

    </form>
  );
}
