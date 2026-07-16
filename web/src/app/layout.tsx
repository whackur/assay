import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: "Assay — Project Intelligence",
  description:
    "Submit a public GitHub repository and read its dimensioned, evidence-grounded project evaluation.",
};

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en">
      <body>
        <a className="skip-link" href="#main">
          Skip to content
        </a>
        <header className="site-header">
          <div className="page">
            <Link className="brand" href="/">
              Assay
            </Link>
            <span className="tagline">Open source project intelligence</span>
          </div>
        </header>
        <main id="main" className="page">
          {children}
        </main>
        <footer className="site-footer">
          <div className="page">
            <Link href="/contact">Contact or report an issue</Link>
          </div>
        </footer>
      </body>
    </html>
  );
}
