"use client";

import { useId, useState } from "react";
import { useRouter } from "next/navigation";

// basePath is the deployment's secret admin base ("/panel-<slug>"); token is
// the one-time setup token that already gated the page render and must be
// presented again to the setup endpoint, where it is consumed.
export function SetupForm({
  basePath,
  token,
}: {
  basePath: string;
  token: string;
}) {
  const router = useRouter();
  const usernameId = useId();
  const passwordId = useId();
  const confirmId = useId();
  const errorId = useId();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    if (password !== confirm) {
      setError("The two passwords do not match.");
      return;
    }
    setPending(true);
    const response = await fetch(`${basePath}/api/setup`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token, username, password }),
    });
    if (response.ok) {
      router.push(basePath);
      router.refresh();
      return;
    }
    const payload = (await response.json().catch(() => null)) as
      | { error?: string }
      | null;
    setError(
      payload?.error ??
        "Setup failed. The one-time token may already be consumed — check the server logs.",
    );
    setPending(false);
  }

  return (
    <form className="auth-form" onSubmit={onSubmit} noValidate>
      <div className="field">
        <label htmlFor={usernameId}>Username</label>
        <input
          id={usernameId}
          name="username"
          type="text"
          autoComplete="username"
          value={username}
          onChange={(event) => setUsername(event.target.value)}
          required
        />
      </div>

      <div className="field">
        <label htmlFor={passwordId}>Password</label>
        <input
          id={passwordId}
          name="password"
          type="password"
          autoComplete="new-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          aria-describedby={error ? errorId : undefined}
          required
        />
        <p className="field-hint">At least 10 characters.</p>
      </div>

      <div className="field">
        <label htmlFor={confirmId}>Confirm password</label>
        <input
          id={confirmId}
          name="confirm"
          type="password"
          autoComplete="new-password"
          value={confirm}
          onChange={(event) => setConfirm(event.target.value)}
          required
        />
      </div>

      {error && (
        <p id={errorId} className="field-error" role="alert">
          {error}
        </p>
      )}

      <button type="submit" disabled={pending}>
        {pending ? "Creating account…" : "Create admin account"}
      </button>
    </form>
  );
}
