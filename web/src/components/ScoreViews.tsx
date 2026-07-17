"use client";

import { useState } from "react";
import type { Scores } from "@/lib/contract/types";
import { DimensionBars, ScoreHero } from "@/components/ScoreCards";
import { ScoreRadar, radarValues } from "@/components/ScoreRadar";
import { DIMENSION_LABELS, scoreDisplay } from "@/lib/state/score-display";

// Toggle between the accurate default reading (dimension bars) and the
// profile reading (the rubric pentagon). The Assay Score hero stays visible in
// both. The radar option only appears when every plotted dimension has a
// released value — a missing score is never drawn as a zero spoke.

type View = "list" | "profile";

export function ScoreViews({ scores }: { scores: Scores }) {
  const [view, setView] = useState<View>("list");
  const radarAvailable = radarValues(scores) !== null;
  const potential = scoreDisplay(scores.potential);

  return (
    <div>
      <ScoreHero score={scores.assay_score} />

      {radarAvailable && (
        <div className="view-toggle" role="group" aria-label="Score view">
          <button
            type="button"
            className="view-toggle-btn"
            aria-pressed={view === "list"}
            onClick={() => setView("list")}
          >
            Breakdown
          </button>
          <button
            type="button"
            className="view-toggle-btn"
            aria-pressed={view === "profile"}
            onClick={() => setView("profile")}
          >
            Profile
          </button>
        </div>
      )}

      {view === "profile" && radarAvailable ? (
        <div>
          <ScoreRadar scores={scores} />
          <p className="radar-footnote">
            {DIMENSION_LABELS.potential} (forward-looking, not part of the
            profile):{" "}
            {potential.hasValue
              ? `${potential.valueText}/100 · confidence ${potential.confidencePercent}% (${potential.confidenceBand})`
              : potential.statusLabel}
          </p>
        </div>
      ) : (
        <DimensionBars scores={scores} />
      )}
    </div>
  );
}
