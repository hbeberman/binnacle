# PRD: Activity Log Enhancements

## Summary

A comprehensive enhancement to the binnacle GUI activity log, transforming it from a simple list of recent actions into a powerful tool for debugging, auditing, and understanding project history. This PRD organizes work into five phases that can be implemented incrementally.

**Related Ideas:** bn-b5c5 (Monitor nodes)

## Problem

The current activity log is minimal:
- Shows only the last 50 entries with no filtering
- No search capability
- No way to understand patterns or trends
- No interactivity with the rest of the graph
- No real-time updates
- Limited usefulness for debugging agent behavior or auditing project history

## Goals

1. **Debugging**: Help users understand what agents did and why things failed
2. **Auditing**: Provide searchable history of all project activity
3. **Insights**: Surface patterns in how work gets done
4. **Integration**: Connect log entries to the entities they affect
5. **Performance**: Handle large log histories without degrading UX

## Non-Goals

- Log aggregation from multiple repositories
- Undo/replay functionality (too dangerous, defer to future)
- Real-time collaboration features
- Log entry editing or deletion (audit trail integrity)

---

## Phase 1: Filtering & Search (Foundation)

**Goal:** Make the existing log useful for finding specific entries.

### 1.1 Command Type Filter

Add a dropdown/chip filter for command categories:
- All (default)
- Tasks (`task create`, `task update`, `task close`, etc.)
- Tests (`test create`, `test run`, etc.)
- Links (`link add`, `link rm`)
- Bugs (`bug create`, `bug update`, etc.)
- Ideas (`idea create`, `idea promote`, etc.)
- Docs (`doc create`, `doc update`, etc.)
- System (`init`, `orient`, `config`, `doctor`, etc.)
- Queue (`queue add`, `queue rm`, etc.)

**Implementation:**
- Client-side filtering of loaded entries
- Multi-select chips (can show Tasks + Bugs together)
- Filter state persists in URL query params

### 1.2 User/Agent Filter

Filter by the `user` field in log entries:
- Dropdown populated from unique users in loaded logs
- Special handling for agent IDs (e.g., `bna-xxxx`)
- "Human only" / "Agents only" quick filters

### 1.3 Success/Failure Toggle

- Three-state toggle: All | Success Only | Failures Only
- Failures highlighted more prominently when mixed

### 1.4 Text Search

- Search box filters entries by:
  - Command name
  - Arguments (entity IDs, titles, etc.)
  - Error messages
- Case-insensitive substring matching
- Debounced (200ms) to avoid excessive re-renders
- Highlight matching text in results

### 1.5 Date Range Picker

- Quick presets: Last hour, Last 24h, Last 7 days, Last 30 days, All time
- Custom date range picker for specific windows
- Requires backend support for pagination (see Phase 4)

### Acceptance Criteria (Phase 1)

- [ ] Command type chips filter the log view
- [ ] User dropdown filters by actor
- [ ] Success/failure toggle works
- [ ] Text search highlights matches
- [ ] Date presets filter entries
- [ ] Filters combine (AND logic)
- [ ] Clear all filters button
- [ ] Filter state reflected in URL

### Test Plan (Phase 1)

- Unit tests for filter logic (command categorization, text matching)
- Integration tests for filter combinations
- Manual: Verify filters work with 100+ entries

---

## Phase 2: Visualization & Insights

**Goal:** Help users understand patterns in project activity.

### 2.1 Activity Timeline

A compact timeline visualization showing activity density over time:
- Horizontal bar spanning the date range
- Color intensity indicates activity volume
- Click on a segment to filter to that time period
- Shows at top of log view

**Implementation:**
- SVG or Canvas rendering
- Bucket entries into time segments (hour/day depending on range)
- Color scale: light (few) → dark (many)

### 2.2 Command Frequency Chart

Collapsible panel showing breakdown of commands:
- Pie or horizontal bar chart
- Shows top 10 command types by frequency
- Click segment to filter to that command type
- Toggle between "All time" and "Current filter"

### 2.3 Agent Activity Summary

When agents are active, show a summary panel:
- Agent ID with session duration
- Commands executed count
- Success/failure rate
- Most common actions
- Expandable to show full agent timeline

### 2.4 Session Grouping

Visually group related actions into "sessions":
- Detect session boundaries (gaps > 5 min or agent change)
- Collapsible session headers showing:
  - Agent/user
  - Duration
  - Action count
  - Summary (e.g., "Worked on bn-1234: 12 actions")
- Expand to see individual entries

**Implementation:**
- Client-side session detection
- Session boundaries configurable
- Collapsed by default for large logs

### Acceptance Criteria (Phase 2)

- [ ] Timeline renders with activity density
- [ ] Clicking timeline segment filters entries
- [ ] Command frequency chart shows distribution
- [ ] Agent summary shows key metrics
- [ ] Sessions group related actions
- [ ] Sessions are collapsible

### Test Plan (Phase 2)

- Unit tests for session boundary detection
- Unit tests for time bucketing
- Visual regression tests for charts
- Manual: Verify charts with varied activity patterns

---

## Phase 3: Interactivity & Navigation

**Goal:** Connect log entries to the entities they affect.

### 3.1 Entity Linking

Make entity IDs in log entries clickable:
- Task IDs (`bn-xxxx`) → Jump to task in graph or detail view
- Test IDs (`bnt-xxxx`) → Jump to test
- Bug IDs, Idea IDs, Doc IDs, Queue IDs
- Highlight the linked node in graph view

**Implementation:**
- Parse entity IDs from `args` field
- Render as styled links
- Smooth scroll/pan to node in graph
- Brief highlight animation on target node

### 3.2 Contextual Log View

When viewing an entity's detail panel, show related log entries:
- "Activity" tab in entity detail view
- Shows all log entries affecting this entity
- Sorted chronologically (newest first)
- Links to full log with filter pre-applied

### 3.3 Copy as CLI Command

Add a "copy" button to each log entry:
- Reconstructs the CLI command from log data
- Copies to clipboard
- Toast notification confirms copy

**Example:**
```
Log entry: { command: "task update", args: { id: "bn-1234", status: "done" } }
Copied: bn task update bn-1234 --status done
```

### 3.4 Entry Detail Expansion

Click entry to expand full details:
- Full arguments (currently truncated)
- Duration breakdown
- Error stack trace (if failed)
- Related entries (same entity, same session)

### 3.5 Keyboard Navigation

- `j`/`k` or arrow keys to navigate entries
- `Enter` to expand selected entry
- `Escape` to collapse / clear selection
- `c` to copy selected entry as CLI command
- `/` to focus search

### Acceptance Criteria (Phase 3)

- [ ] Entity IDs are clickable links
- [ ] Clicking entity ID navigates to graph/detail
- [ ] Entity detail view has Activity tab
- [ ] Copy button generates valid CLI command
- [ ] Entries expand to show full details
- [ ] Keyboard navigation works

### Test Plan (Phase 3)

- Unit tests for CLI command reconstruction
- Unit tests for entity ID parsing
- Integration tests for navigation
- Manual: Verify links work across entity types

---

## Phase 4: Real-time & Performance

**Goal:** Handle large logs and live updates efficiently.

### 4.1 Pagination & Infinite Scroll

Replace fixed 50-entry limit with pagination:
- Initial load: 100 entries
- Scroll to bottom loads more (infinite scroll)
- "Load more" button as fallback
- Total count shown (e.g., "Showing 100 of 5,432 entries")

**Backend changes:**
- `GET /api/log?limit=100&offset=0&before=<timestamp>`
- Support filtering on server side for large logs
- Index log file for efficient seeking (or migrate to SQLite)

### 4.2 Live Streaming Updates

Real-time log updates via WebSocket:
- New entries appear at top of log
- Subtle animation for new entries
- Pause auto-scroll when user is scrolled up
- "New entries" badge when paused
- Click badge to jump to latest

**Implementation:**
- Extend existing WebSocket connection
- New message type: `log_entry`
- Server watches log file for changes
- Debounce to batch rapid entries

### 4.3 Log Retention Settings

Configuration for log management:
- `bn config set action_log_max_entries 10000`
- `bn config set action_log_max_age_days 90`
- `bn log compact` to apply retention rules
- Show retention settings in GUI

### 4.4 Performance Optimization

- Virtual scrolling for large entry lists
- Lazy rendering of expanded entry details
- Web Worker for filtering/search on large datasets
- Compression for log storage

### Acceptance Criteria (Phase 4)

- [ ] Infinite scroll loads more entries
- [ ] Backend supports pagination params
- [ ] WebSocket streams new entries
- [ ] Auto-scroll pauses when scrolled up
- [ ] Retention settings configurable
- [ ] UI remains responsive with 10k+ entries

### Test Plan (Phase 4)

- Load tests with 10k, 50k, 100k entries
- WebSocket connection resilience tests
- Pagination boundary tests
- Manual: Verify smooth scroll with large logs

---

## Phase 5: Contextual Integration

**Goal:** Deep integration with the rest of binnacle.

### 5.1 Commit Correlation

Link log entries to git commits:
- Show commit SHA when action occurred during a commit
- "View related commits" for entity-affecting actions
- Timeline overlay showing commits + log entries

**Implementation:**
- Cross-reference timestamps with commit history
- Use existing `bn commit list` data
- Visual markers on timeline for commits

### 5.2 Entry Annotations

Allow adding notes to log entries:
- "Add note" button on expanded entry
- Notes stored separately (don't modify log)
- Searchable alongside entry content
- Use case: "This failure was due to network issue, not code bug"

**Implementation:**
- New storage: `annotations.jsonl`
- Keyed by log entry timestamp + command hash
- Render in expanded entry view

### 5.3 Export & Reporting

Export filtered logs for external analysis:
- Export as JSON, CSV, or Markdown
- Include current filter in export
- Generate summary reports:
  - Daily/weekly activity digest
  - Agent performance metrics
  - Error frequency analysis

### 5.4 Monitor Node Integration

Connect to bn-b5c5 (Monitor nodes) when implemented:
- Activity log as a type of monitor node
- Embed mini activity feed in graph
- Agent nodes show their activity inline

### Acceptance Criteria (Phase 5)

- [ ] Commits appear on activity timeline
- [ ] Annotations persist and are searchable
- [ ] Export generates valid JSON/CSV
- [ ] Summary report generation works
- [ ] Monitor node integration ready (when bn-b5c5 ships)

### Test Plan (Phase 5)

- Unit tests for export format generation
- Integration tests for annotation CRUD
- Manual: Verify commit correlation accuracy

---

## Technical Architecture

### Data Flow

```
action.log (JSONL) 
    ↓
GET /api/log (paginated, filtered)
    ↓
WebSocket stream (new entries)
    ↓
Client state (filtered, sorted)
    ↓
Virtual list renderer
```

### New API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/log` | GET | Paginated log entries with filters |
| `/api/log/stats` | GET | Aggregated statistics for charts |
| `/api/log/export` | GET | Export filtered entries |
| `/api/annotations` | GET/POST | Log entry annotations |
| (WebSocket) | - | `log_entry` message type |

### New Config Keys

| Key | Default | Description |
|-----|---------|-------------|
| `action_log_max_entries` | 50000 | Max entries before rotation |
| `action_log_max_age_days` | 90 | Max age before cleanup |
| `action_log_index_enabled` | true | Enable SQLite index for search |

### Storage Changes

- **Phase 1-3:** No backend changes, client-side only
- **Phase 4:** Add SQLite index for efficient queries
- **Phase 5:** Add `annotations.jsonl` file

---

## Implementation Order & Dependencies

```
Phase 1 (Foundation)
    ├── 1.1 Command filter (standalone)
    ├── 1.2 User filter (standalone)  
    ├── 1.3 Success toggle (standalone)
    ├── 1.4 Text search (standalone)
    └── 1.5 Date range (needs backend for full history)
            ↓
Phase 2 (Visualization)
    ├── 2.1 Timeline (depends on date range)
    ├── 2.2 Command chart (depends on filters)
    ├── 2.3 Agent summary (depends on user filter)
    └── 2.4 Session grouping (standalone)
            ↓
Phase 3 (Interactivity)
    ├── 3.1 Entity linking (standalone)
    ├── 3.2 Contextual view (depends on entity linking)
    ├── 3.3 Copy as CLI (standalone)
    ├── 3.4 Entry expansion (standalone)
    └── 3.5 Keyboard nav (depends on entry expansion)
            ↓
Phase 4 (Performance)
    ├── 4.1 Pagination (backend required)
    ├── 4.2 Live streaming (depends on WebSocket)
    ├── 4.3 Retention (backend required)
    └── 4.4 Optimization (depends on pagination)
            ↓
Phase 5 (Integration)
    ├── 5.1 Commit correlation (depends on pagination)
    ├── 5.2 Annotations (standalone storage)
    ├── 5.3 Export (depends on filters)
    └── 5.4 Monitor nodes (external dependency)
```

---

## Rollout Strategy

1. **Phase 1:** Ship immediately, client-side only, no risk
2. **Phase 2:** Ship after Phase 1 stabilizes, still client-side
3. **Phase 3:** Ship incrementally, entity linking first
4. **Phase 4:** Requires backend changes, more testing needed
5. **Phase 5:** Ship features independently as completed

Feature flags:
- `gui.activity_log.v2` - Enables Phase 1-3 features
- `gui.activity_log.streaming` - Enables Phase 4 WebSocket
- `gui.activity_log.annotations` - Enables Phase 5 annotations

---

## Success Metrics

- **Adoption:** % of GUI sessions that use log filters
- **Debugging:** Time to identify cause of agent failures (qualitative)
- **Performance:** P95 render time stays under 100ms with 1000+ entries
- **Satisfaction:** User feedback on log usefulness

---

## Open Questions

1. Should session grouping be opt-in or default?
2. What's the right default retention period?
3. Should annotations be shared (synced) or local-only?
4. Priority of monitor node integration vs other Phase 5 work?

---

## Appendix: Current Implementation Reference

### Log Entry Schema
```json
{
  "timestamp": "2026-01-25T02:00:00Z",
  "repo_path": "/path/to/repo",
  "command": "task update",
  "args": { "id": "bn-1234", "status": "done" },
  "success": true,
  "error": null,
  "duration_ms": 42,
  "user": "bna-e6e8"
}
```

### Current Endpoints
- `GET /api/log` - Returns last 100 entries (no pagination)

### Current UI
- Shows last 50 entries
- Red highlight for failures
- No filtering or search
