import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  reactStrictMode: true,
  poweredByHeader: false,
  output: "standalone",
  env: {
    NEXT_PUBLIC_COMMIT_SHA:
      process.env.GIT_COMMIT_SHA ?? process.env.GITHUB_SHA ?? "unknown",
  },
};

export default nextConfig;
