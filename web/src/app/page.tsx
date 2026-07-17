import { SubmissionForm } from "@/components/SubmissionForm";
import { ProjectNotice } from "@/components/ProjectNotice";
import { FeaturedCard } from "@/components/FeaturedCard";
import { CatalogBrowser } from "@/components/CatalogBrowser";
import { SampleReport, TraceChain } from "@/components/SampleReport";
import { filterOptions } from "@/lib/catalog/catalog";
import { featuredEntries, publicCatalogEntries } from "@/lib/catalog/fixtures";
import { defaultDataDir, hiddenEntryIds } from "@/lib/admin/store";

export const dynamic = "force-dynamic";

export default async function HomePage() {
  // Admin-hidden entries drop out of the public lists; the records themselves
  // are untouched and their evaluation pages stay reachable by direct link.
  const hidden = new Set(await hiddenEntryIds(defaultDataDir()));
  const entries = publicCatalogEntries().filter((entry) => !hidden.has(entry.id));
  const featured = featuredEntries().filter((entry) => !hidden.has(entry.id));
  const options = filterOptions(entries);

  return (
    <div>
      <section className="hero">
        <p className="hero-kicker">assay, n. — the testing of a metal to
        determine its purity.</p>
        <h1>
          How good is your code, <span className="accent-word">really</span>?
        </h1>
        <p className="hero-lede">
          Paste a public GitHub repository. Assay reads the evidence — files,
          features, the snapshot itself — and hands down a scored verdict.{" "}
          <strong>Every claim cited. No hand-waving.</strong>
        </p>
        <SubmissionForm />
        <p className="hero-honesty">
          Preview deployment: submissions resolve against fixture evaluations
          from the versioned report contract, not a live analysis engine. The
          report you see is the real output format.
        </p>
      </section>

      <section className="section" aria-labelledby="specimen">
        <div className="section-head">
          <h2 id="specimen">The verdict you get</h2>
          <p>
            Not a star rating. A scored, versioned document with confidence
            bands, graded evidence, and a hard release gate — a score that
            cannot be backed by evidence is withheld, never faked as a zero.
          </p>
        </div>
        <SampleReport />
      </section>

      <section className="section" aria-labelledby="traceable">
        <div className="trace">
          <div>
            <div className="section-head">
              <h2 id="traceable">Every score is traceable</h2>
            </div>
            <div className="stack prose">
              <p>
                Deterministic collectors gather graded evidence from the
                repository; an AI evaluator judges it against a versioned
                rubric; a compiler turns the judgments into scores. Walk any
                number in the report back to the commit-pinned record it
                stands on. If it can&rsquo;t be cited, it isn&rsquo;t scored.
              </p>
              <p className="muted">
                On the right: one real chain from the specimen above — a
                dimension score, the evidence id it cites, and the graded
                record behind that id.
              </p>
              <ProjectNotice />
            </div>
          </div>
          <TraceChain />
        </div>
      </section>

      <section className="section" aria-labelledby="catalog-heading" id="catalog">
        <div className="section-head">
          <h2 id="catalog-heading">See where you&rsquo;d land</h2>
          <p>
            Anonymous evaluations publish automatically. Featured projects are
            editorially selected and labeled; featuring never buys a point.
          </p>
        </div>

        <div className="featured-grid" style={{ marginBottom: "var(--space-lg)" }}>
          {featured.map((entry) => (
            <FeaturedCard key={entry.id} entry={entry} />
          ))}
        </div>

        <CatalogBrowser entries={entries} options={options} />
      </section>
    </div>
  );
}
