import { notFound } from "next/navigation";
import { fixtureApi } from "@/lib/api/client";
import { ResultView } from "@/components/ResultView";
import { ProgressPanel } from "@/components/ProgressPanel";
import { isPublicResult } from "@/lib/state/result-state";

export default async function EvaluationPage({
  params,
}: {
  params: Promise<{ slug: string[] }>;
}) {
  const { slug } = await params;
  const id = slug.join("/");
  const record = await fixtureApi.getRecord(id);

  if (!record) notFound();

  if (record.state === "in_flight") {
    return <ProgressPanel job={record.job} />;
  }

  // The public route only serves published public results (OPI-013).
  if (!isPublicResult(record.evaluation)) notFound();

  const comparison = await fixtureApi.getComparison(id);
  return (
    <ResultView
      evaluation={record.evaluation}
      evidence={record.evidence}
      comparison={comparison}
    />
  );
}
