# PRD: GUI Graph Search Filter

## Summary
Add a search bar to the GUI graph view that filters the rendered graph to only show nodes matching the query, hiding nodes that do not match.

## Problem
In medium/large repos, the graph view becomes visually dense. Users need a quick way to narrow the graph to relevant tasks/tests/ideas without changing underlying data.

## Goals
- Provide a search input in the **Graph** view.
- While typing, only render nodes that match the query.
- Non-matching nodes should be hidden (not deleted).
- Matching should include at least:
  - Node ID (e.g., `bn-xxxx`, `bnt-xxxx`)
  - Node title/name (task title, test name, etc.)

## Non-Goals
- Full-text search across logs/notes.
- Advanced query language (AND/OR, regex, tags, status filters).
- Persisting the search query across page reloads.

## UX Requirements
- Search bar appears only in Graph view (not other views).
- Placeholder text: `Search nodes…`
- Case-insensitive substring matching.
- Empty query shows the full graph (current behavior).
- Optional (nice-to-have): Escape clears the search input.

## Graph Filtering Behavior
- Filter affects rendering only.
- If a node matches, it is shown regardless of its neighbors.
- Edges are shown only when **both** endpoints are visible.
- If filtering results in 0 nodes, show an empty-state message near the canvas (or a small overlay): `No matching nodes`.

## Performance
- Filtering should be fast enough for interactive typing on graphs with hundreds of nodes.
- Use debouncing only if necessary; start with immediate filtering.

## Acceptance Criteria
- Given nodes A, B, C where only B matches the query, the graph shows only B.
- Clearing the query restores all nodes.
- Matching works for both IDs and titles.
- No backend/storage changes required.

## Test Plan
- Unit test (JS or Rust, depending on where filtering logic lives) for match + filter logic:
  - Case-insensitive substring match
  - Empty query returns all nodes
  - Edge filtering removes edges with hidden endpoint
- Manual:
  - Run `just gui`, open Graph view, type partial task ID, confirm only those nodes remain.

## Rollout
- Ship behind existing GUI feature flag (no new flags).
