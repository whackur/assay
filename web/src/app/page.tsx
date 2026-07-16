import Link from "next/link";
import { SubmissionForm } from "@/components/SubmissionForm";
import { ProjectNotice } from "@/components/ProjectNotice";

export default function HomePage() {
  return (
    <div className="stack">
      <h1>Evaluate a public GitHub project</h1>
      <p className="muted">
        Enter a repository URL. A cached result opens immediately; a new
        submission starts an asynchronous evaluation.
      </p>

      <SubmissionForm />
      <ProjectNotice />

      <section>
        <h2>Example results</h2>
        <ul>
          <li>
            <Link href="/evaluations/example-org/sample-project">
              example-org/sample-project
            </Link>{" "}
            <span className="muted">— complete, anonymous public result</span>
          </li>
          <li>
            <Link href="/evaluations/example-org/early-prototype">
              example-org/early-prototype
            </Link>{" "}
            <span className="muted">— partial, insufficient release gate</span>
          </li>
          <li>
            <Link href="/evaluations/acme/degraded">acme/degraded</Link>{" "}
            <span className="muted">
              — authenticated preview, provider unavailable
            </span>
          </li>
          <li>
            <Link href="/evaluations/acme/in-progress">acme/in-progress</Link>{" "}
            <span className="muted">— in-flight analysis progress</span>
          </li>
        </ul>
      </section>
    </div>
  );
}
