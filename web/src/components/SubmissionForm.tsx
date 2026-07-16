"use client";

import Link from "next/link";
import { useId, useMemo, useState } from "react";
import { useRouter } from "next/navigation";
import { fixtureApi, type SubmissionOutcome } from "@/lib/api/client";
import { parseGithubTarget } from "@/lib/state/github-url";

type CooldownOutcome = Extract<SubmissionOutcome, { kind: "cooldown" }>;

// Submission form and live canonical preview (specification 12.5). Host
// validation runs client-side to guide the user; the server repeats it.

export function SubmissionForm() {
  const router = useRouter();
  const inputId = useId();
  const previewId = useId();
  const errorId = useId();
  const cooldownId = useId();
  const [value, setValue] = useState("");
  const [submitError, setSubmitError] = useState<string | null>(null);
  const [cooldown, setCooldown] = useState<CooldownOutcome | null>(null);
  const [pending, setPending] = useState(false);

  const preview = useMemo(() => {
    if (value.trim() === "") return null;
    return parseGithubTarget(value);
  }, [value]);

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    setPending(true);
    setSubmitError(null);
    setCooldown(null);
    const outcome = await fixtureApi.submit(value);
    if (outcome.kind === "invalid") {
      setSubmitError(outcome.error);
      setPending(false);
      return;
    }
    if (outcome.kind === "cooldown") {
      setCooldown(outcome);
      setPending(false);
      return;
    }
    router.push(`/evaluations/${outcome.id}`);
  }

  const describedBy = [
    preview ? previewId : null,
    submitError ? errorId : null,
    cooldown ? cooldownId : null,
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

      <div aria-live="polite">
        {cooldown && (
          <div id={cooldownId} className="notice" role="status">
            This repository was already analyzed on the{" "}
            {cooldown.cooldown.profile} profile. A refresh is on cooldown; the
            next eligible analysis is in {cooldown.cooldown.remainingLabel} (
            {cooldown.cooldown.nextEligibleAt.slice(0, 10)}). You can open the
            existing result now.{" "}
            <Link href={`/evaluations/${cooldown.id}`}>View cached result</Link>
          </div>
        )}
      </div>

      <button type="submit" disabled={pending}>
        {pending ? "Submitting…" : "Analyze repository"}
      </button>
    </form>
  );
}
