import type { ProjectEvidence } from "@/lib/contract/types";
import { evidenceLabel, groupEvidence } from "@/lib/state/evidence";

function gradeLabel(grade: ProjectEvidence["grade"]): string {
  return grade ? `grade ${grade.toUpperCase()}` : "no grade";
}

export function EvidenceExplorer({ evidence }: { evidence: ProjectEvidence[] }) {
  const groups = groupEvidence(evidence);
  return (
    <div className="stack">
      {groups.map((group) => (
        <details key={group.kind} className="evidence-group card" open>
          <summary>
            {group.label} ({group.items.length})
          </summary>
          {group.items.map((item) => (
            <div key={item.id} className="evidence-item">
              <span>
                {evidenceLabel(item)}
                <br />
                <span className="evidence-id">{item.id}</span>
              </span>
              <span className="muted">
                {item.status} · {gradeLabel(item.grade)}
              </span>
            </div>
          ))}
        </details>
      ))}
    </div>
  );
}
