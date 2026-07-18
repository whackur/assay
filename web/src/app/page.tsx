import { SubmissionForm } from "@/components/SubmissionForm";
import { ProjectNotice } from "@/components/ProjectNotice";
import { SampleReport, TraceChain } from "@/components/SampleReport";
import { LiveSourceActivity } from "@/components/LiveCatalog";
import { getHostedRecentSources } from "@/lib/api/hosted";

export const dynamic = "force-dynamic";

export default async function HomePage() {
  const recentSources = await getHostedRecentSources();

  return (
    <div>
      <section className="hero">
        <p className="hero-kicker">assay, n. — the testing of a metal to
        determine its purity.</p>
        <h1>
          Track a public repository, <span className="accent-word">honestly</span>.
        </h1>
        <p className="hero-lede">
          Submit a public GitHub repository. This hosted slice records its
          stable GitHub identity, normalized public metadata, and immutable
          default-branch revision. <strong>It does not publish a project score.</strong>
        </p>
        <SubmissionForm />
        <p className="hero-honesty">
          Hosted workflow: submissions are queued in PostgreSQL, resolved to a
          stable GitHub repository identity, and collected by a leased worker.
          Temporary failures retry with bounded backoff; missing data stays
          unavailable rather than becoming zero.
        </p>
      </section>

      <section className="section" aria-labelledby="specimen">
        <div className="section-head">
          <h2 id="specimen">Sample report format</h2>
          <p>
            Not a star rating. A scored, versioned document with confidence
            bands, graded evidence, and a hard release gate — a score that
            cannot be backed by evidence is withheld, never faked as a zero.
          </p>
        </div>
        <div className="notice" role="note" style={{ marginBottom: "var(--space-lg)" }}>
          Sample only: this versioned fixture demonstrates the final report
          contract. It is not a live result and is not mixed into source-processing status.
        </div>
        <SampleReport />
      </section>

      <section className="section" aria-labelledby="traceable">
        <div className="trace">
          <div>
            <div className="section-head">
              <h2 id="traceable">What the sample contract requires</h2>
            </div>
            <div className="stack prose">
              <p>
                The labeled sample demonstrates Assay&rsquo;s full report
                contract: deterministic evidence, canonical AI validation,
                and compiler-produced scores. The live hosted collector below
                has not completed that pipeline and publishes no judgment or
                score.
              </p>
              <p className="muted">
                On the right: one sample chain from the specimen above — a
                dimension score, the evidence id it cites, and the graded
                record behind that id.
              </p>
              <ProjectNotice />
            </div>
          </div>
          <TraceChain />
        </div>
      </section>

      <section className="section" aria-labelledby="source-activity-heading" id="source-activity">
        <div className="section-head">
          <h2 id="source-activity-heading">Recent source processing</h2>
          <p>
            Live PostgreSQL-backed repository status. This slice stores
            normalized GitHub metadata and revision provenance only. Provider
            transport output is not published as a judgment or score.
          </p>
        </div>

        <LiveSourceActivity result={recentSources} />
      </section>
    </div>
  );
}
