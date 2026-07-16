import { SubmissionForm } from "@/components/SubmissionForm";
import { ProjectNotice } from "@/components/ProjectNotice";
import { FeaturedCard } from "@/components/FeaturedCard";
import { CatalogBrowser } from "@/components/CatalogBrowser";
import { filterOptions } from "@/lib/catalog/catalog";
import { featuredEntries, publicCatalogEntries } from "@/lib/catalog/fixtures";

export default function HomePage() {
  const entries = publicCatalogEntries();
  const featured = featuredEntries();
  const options = filterOptions(entries);

  return (
    <div className="stack">
      <h1>Assay public catalog</h1>
      <p className="muted">
        Anonymous evaluations publish automatically. Featured projects are
        editorially selected and labeled; featuring does not affect any score.
      </p>

      <section aria-labelledby="featured">
        <h2 id="featured">Featured projects</h2>
        <div className="featured-grid">
          {featured.map((entry) => (
            <FeaturedCard key={entry.id} entry={entry} />
          ))}
        </div>
      </section>

      <CatalogBrowser entries={entries} options={options} />

      <section aria-labelledby="submit">
        <h2 id="submit">Evaluate a public GitHub project</h2>
        <p className="muted">
          Enter a repository URL. A cached result opens immediately; a new
          submission starts an asynchronous evaluation.
        </p>
        <SubmissionForm />
      </section>

      <ProjectNotice />
    </div>
  );
}
