import type { Metadata } from "next";
import Link from "next/link";
import { Fraunces, IBM_Plex_Sans, IBM_Plex_Mono } from "next/font/google";
import "./globals.css";

const fraunces = Fraunces({
  subsets: ["latin"],
  variable: "--font-fraunces",
  axes: ["opsz"],
});

const plexSans = IBM_Plex_Sans({
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  variable: "--font-plex-sans",
});

const plexMono = IBM_Plex_Mono({
  subsets: ["latin"],
  weight: ["400", "500"],
  variable: "--font-plex-mono",
});

export const metadata: Metadata = {
  title: "Assay — evidence-based project evaluation",
  description:
    "Paste a public GitHub repository and read its dimensioned, evidence-cited project evaluation. Every score is traceable.",
};

// Nothing in the public chrome reads deployment state: the admin area lives
// only under its secret per-deployment path (see src/lib/admin/panel.ts), so
// the public site looks identical whether or not an administrator exists.

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html
      lang="en"
      className={`${fraunces.variable} ${plexSans.variable} ${plexMono.variable}`}
    >
      <body>
        <a className="skip-link" href="#main">
          Skip to content
        </a>
        <header className="masthead">
          <div className="shell masthead-inner">
            <Link className="brand" href="/">
              Assay
            </Link>
            <span className="brand-tag">evidence-based project evaluation</span>
            <nav className="masthead-nav" aria-label="Site">
              <Link href="/#catalog">Catalog</Link>
              <Link href="/contact">Contact</Link>
              <a
                className="github-link"
                href="https://github.com/whackur/assay"
                target="_blank"
                rel="noopener noreferrer"
                aria-label="Assay source on GitHub"
              >
                <svg
                  className="github-mark"
                  viewBox="0 0 16 16"
                  width="18"
                  height="18"
                  aria-hidden="true"
                  fill="currentColor"
                >
                  <path d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27s1.36.09 2 .27c1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.01 8.01 0 0 0 16 8c0-4.42-3.58-8-8-8Z" />
                </svg>
                <span className="github-word">GitHub</span>
              </a>
            </nav>
          </div>
        </header>
        <main id="main" className="shell">
          {children}
        </main>
        <footer className="colophon">
          <div className="shell">
            <div>
              <div className="colophon-brand">Assay</div>
              <p>
                An evidence-based evaluation engine for public repositories.
                Scores are dimensioned, versioned, and grounded in cited
                evidence — never in impressions.
              </p>
              <p>
                Assay reads public repositories only. Private project? Publish
                a public fork or mirror, then submit that.
              </p>
            </div>
            <div>
              <p>
                <Link href="/contact">Contact or report an issue</Link>
              </p>
            </div>
            <div className="mono">
              <p>
                report contract v1 · evaluation project-intelligence-1
                <br />
                preview deployment — results render from fixture evaluations,
                not a live engine
              </p>
            </div>
          </div>
        </footer>
      </body>
    </html>
  );
}
