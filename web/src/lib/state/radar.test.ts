import assert from "node:assert/strict";
import { test } from "node:test";
import {
  labelAnchor,
  polygonPoints,
  radarVertex,
  ringPoints,
} from "@/lib/state/radar";

test("the first axis points straight up", () => {
  const p = radarVertex(0, 5, 1, 100, 170, 150);
  assert.equal(p.x, 170);
  assert.equal(p.y, 50);
});

test("a zero value sits at the center", () => {
  const p = radarVertex(3, 5, 0, 100, 170, 150);
  assert.equal(p.x, 170);
  assert.equal(p.y, 150);
});

test("polygon output is deterministic and ordered", () => {
  const a = polygonPoints([80, 60, 90, 70, 50], 100, 170, 150);
  const b = polygonPoints([80, 60, 90, 70, 50], 100, 170, 150);
  assert.equal(a, b);
  assert.equal(a.split(" ").length, 5);
  assert.equal(a.split(" ")[0], "170,70");
});

test("a full ring equals a polygon of 100s", () => {
  assert.equal(
    ringPoints(1, 5, 100, 170, 150),
    polygonPoints([100, 100, 100, 100, 100], 100, 170, 150),
  );
});

test("rejects degenerate radars", () => {
  assert.throws(() => radarVertex(0, 2, 1, 100, 0, 0));
  assert.throws(() => radarVertex(5, 5, 1, 100, 0, 0));
});

test("label anchors split by side", () => {
  // Pentagon: top, upper-right, lower-right, lower-left, upper-left.
  assert.equal(labelAnchor(0, 5), "middle");
  assert.equal(labelAnchor(1, 5), "start");
  assert.equal(labelAnchor(2, 5), "start");
  assert.equal(labelAnchor(3, 5), "end");
  assert.equal(labelAnchor(4, 5), "end");
});
