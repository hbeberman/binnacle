# Idea: Distributed Subgraph Coordination

**Status:** Early exploration  
**Created:** 2026-01-24

## Problem Statement

When multiple agents or DRIs (Directly Responsible Individuals) work on different parts of a task graph simultaneously, there's no mechanism for:
- Claiming ownership of a subgraph
- Protecting in-flight work from external changes
- Coordinating merges that touch owned subgraphs
- Queuing operations that need to wait for a subgraph to be released

## Current Architecture

- **JSONL**: Append-only event log (source of truth)
- **SQLite**: Derived cache for fast queries
- **Assumption**: Single-writer or cooperative multi-writer with soft locks via task status

## Proposed Concepts

### Subgraph Boundaries
How do we define what belongs to a subgraph?
- Tree root (all descendants of a milestone/epic)
- Explicit membership (tagged entities)
- Dynamic (query-based membership)

### Lock Semantics
```
Subgraph checkout:
  - DRI claims ownership with lease/heartbeat
  - Lock has expiry to prevent abandoned locks
  - Read always allowed, write requires permission
```

### Coordination Protocol
When merge controller receives a write to a locked subgraph:
1. Check lock status
2. Query DRI: "Can I merge X into your subgraph?"
3. DRI responds: ALLOW / DENY / WAIT
4. If WAIT → queue merge, poll for unlock
5. On unlock → canonicalize subgraph → apply queued operations

### Open Questions

1. **Transport**: How does merge controller talk to DRI? (webhook, polling, message queue, filesystem watch?)
2. **Conflict resolution**: What if DRI is unreachable? Timeout and force? Escalate?
3. **Granularity**: Lock entire subgraph or individual entities?
4. **Storage**: Where does lock state live? (JSONL event? Separate coordination layer?)
5. **Offline**: Can DRI work fully offline and sync later?

## Why This Is Hard

This moves binnacle from:
- **Single-writer append log** → **Distributed graph with ownership**

Similar systems for reference:
- Git (refs as locks, merge commits as coordination)
- CRDTs (eventual consistency without coordination)
- Distributed databases (Raft/Paxos for consensus)

## Simpler Alternatives (for limited use cases)

| Pain Point | Simpler Solution |
|------------|------------------|
| Agents colliding | Claim tasks explicitly (`in_progress` = soft lock) |
| Protecting mid-flight work | Optimistic locking with version checks |
| True delegation/ownership | Needs the full coordination machinery |

## Next Steps

- [ ] Identify concrete use cases that aren't solved by current soft-locking
- [ ] Sketch minimal viable coordination protocol
- [ ] Evaluate CRDT approach vs explicit coordination
- [ ] Consider whether this belongs in core binnacle or as an extension layer
