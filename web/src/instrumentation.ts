// Runs once per server start (dev and production). Prints the first-run
// setup banner — secret admin path plus one-time setup token — to the server
// console while no administrator is configured.
export async function register(): Promise<void> {
  if (process.env.NEXT_RUNTIME !== "nodejs") return;
  // Skip the compile-time pass of `next build`; the banner belongs to a
  // running server, not to the build environment.
  if (process.env.NEXT_PHASE === "phase-production-build") return;
  const { printFirstRunBannerIfNeeded } = await import(
    "@/lib/admin/bootstrap"
  );
  await printFirstRunBannerIfNeeded();
}
