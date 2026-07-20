import Link from "next/link";
import { notFound } from "next/navigation";
import { getHostedProject, getHostedProjectAiAnalysis } from "@/lib/api/hosted";
import { LiveProjectStatus } from "@/components/LiveCatalog";
import { ProjectAiAnalysisSection } from "@/components/ProjectAiAnalysis";

export const dynamic = "force-dynamic";

export default async function HostedProjectPage({ params }: { params: Promise<{ owner: string; repository: string }> }) {
  const { owner, repository } = await params;
  const result = await getHostedProject(owner, repository);
  if (result.state === "unavailable" && result.reason === "not_found") notFound();
  if (result.state === "unavailable") return <section className="section"><h1>Project status unavailable</h1><div className="notice" role="status">The hosted API could not be reached. Assay does not convert missing data to a zero.</div></section>;
  const project = result.data;
  const aiAnalysis = await getHostedProjectAiAnalysis(owner, repository);
  return <section className="section">
    <p className="hero-kicker">live source-processing status</p>
    <h1>{project.owner}/{project.repository}</h1>
    <p className="hero-lede">{project.description ?? "GitHub description unavailable."}</p>
    <p><a href={project.canonical_url}>Open canonical GitHub repository</a></p>
    <LiveProjectStatus initial={project} />
    {aiAnalysis.state === "available" ? <ProjectAiAnalysisSection analysis={aiAnalysis.data} /> : <div className="notice" role="status">{project.evaluation_status === "validated_unpublished" ? "AI analysis was validated but withheld from publication by the safety gate." : project.evaluation_status === "partial" ? "AI analysis is pending while the source evaluation completes." : "AI analysis is currently unavailable for this project."}</div>}
    <div className="notice" role="note" style={{ marginTop: "var(--space-lg)" }}>This live page reports ingested GitHub metadata and workflow status only. It does not indicate publication approval or catalog inclusion. The scored report shown on the home page is a labeled sample fixture.</div>
    <p style={{ marginTop: "var(--space-lg)" }}><Link href="/">Back to source activity</Link></p>
  </section>;
}
