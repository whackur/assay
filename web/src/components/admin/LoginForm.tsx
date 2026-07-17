"use client";

import { useId, useState } from "react";
import { useRouter } from "next/navigation";

// basePath is the deployment's secret admin base ("/panel-<slug>"); the form
// only ever talks to endpoints under it.
export function LoginForm({ basePath }: { basePath: string }) {
  const router = useRouter();
  const usernameId = useId();
  const passwordId = useId();
  const errorId = useId();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);

  async function onSubmit(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    setPending(true);
    const response = await fetch(`${basePath}/api/login`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ username, password }),
    });
    if (response.ok) {
      router.push(basePath);
      router.refresh();
      return;
    }
    const payload = (await response.json().catch(() => null)) as
      | { error?: string; setupRequired?: boolean }
      | null;
    // setupRequired: no admin exists yet. The setup page needs the one-time
    // token from the server console, so show the pointer instead of
    // redirecting to a page that would 404 without it.
    setError(payload?.error ?? "Sign-in failed. Check the server logs.");
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
          autoComplete="current-password"
          value={password}
          onChange={(event) => setPassword(event.target.value)}
          aria-describedby={error ? errorId : undefined}
          required
        />
      </div>

      {error && (
        <p id={errorId} className="field-error" role="alert">
          {error}
        </p>
      )}

      <button type="submit" disabled={pending}>
        {pending ? "Signing in…" : "Sign in"}
      </button>
    </form>
  );
}
