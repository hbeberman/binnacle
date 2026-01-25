# PRD: Active Task Timer

## Overview

Add a "working for" duration counter to the Active Task card in the GUI main screen. The counter shows how long the current task has been in the active/in_progress state, with a subtle visual warning when it exceeds a threshold (15 minutes by default).

**Related ideas:** None directly related. This is a new usability feature for the GUI.

## Problem Statement

Users currently have no visibility into how long a task has been active. This makes it easy to lose track of time and miss signs that:
- An agent may be stuck or spinning
- A task is taking longer than expected
- The session has gone stale

A simple elapsed time counter with a gentle warning helps users notice when something might need attention.

## Scope

### In Scope
- Elapsed time counter displayed in the Active Task card
- Timer starts when a task becomes the "active task" (shown in the Active Task area)
- Subtle visual warning (dim red/orange glow) after 15-minute threshold
- GUI-only tracking (no persistence to backend/task data)
- Counter resets when active task changes

### Out of Scope (Future)
- Persistent time tracking on task entities
- Configurable threshold (hardcoded 15 min for v1)
- Session logging/history
- Time estimates vs actual comparison
- CLI commands for time tracking

## User Experience

### Display Location
The timer appears in the **Active Task card** on the main GUI screen, below or alongside the task title.

### Display Format
```
Working for: 12m 34s
```

For longer durations:
```
Working for: 1h 23m
```

### Visual States

1. **Normal (0-15 min):** Standard text color, no special styling
2. **Warning (>15 min):** Dim red/orange glow on the timer text or card border - subtle but noticeable

### Behavior
- Timer starts at 0:00 when a task appears in the Active Task card
- Updates every second
- Resets to 0:00 when the active task changes
- Threshold warning appears at exactly 15:00 and persists until task changes

## Technical Design

### Implementation Approach

**Frontend-only (JavaScript/WASM):**
- Store `activeTaskStartTime: Option<Instant>` in GUI state
- On each render tick, calculate elapsed = now - startTime
- Apply warning CSS class when elapsed > 15 min

### State Management

```rust
// In GUI state
struct ActiveTaskState {
    task_id: Option<String>,
    started_at: Option<Instant>,  // When this task became active
}

impl ActiveTaskState {
    fn elapsed(&self) -> Option<Duration> {
        self.started_at.map(|t| Instant::now() - t)
    }
    
    fn is_warning(&self) -> bool {
        self.elapsed().map(|d| d > Duration::from_secs(15 * 60)).unwrap_or(false)
    }
}
```

### CSS Styling

```css
.active-task-timer {
    font-family: monospace;
    font-size: 0.9em;
    color: var(--text-secondary);
}

.active-task-timer.warning {
    color: #ff6b6b;
    text-shadow: 0 0 8px rgba(255, 107, 107, 0.4);
}
```

## Tasks

This feature should be broken into subtasks:

1. **Add timer state tracking** - Store start time when active task changes
2. **Implement elapsed time calculation** - Calculate and format duration
3. **Add timer display to Active Task card** - Render the "Working for: Xm Ys" text
4. **Add warning styling** - CSS for dim red/orange glow after threshold
5. **Wire up tick/update loop** - Ensure timer updates every second

## Testing

### Manual Testing
- [ ] Timer shows 0:00 when task first becomes active
- [ ] Timer increments correctly (spot check at 1m, 5m)
- [ ] Timer resets when active task changes
- [ ] Warning glow appears at 15:00
- [ ] Warning persists after 15 min
- [ ] No timer shown when no active task

### Unit Tests
- [ ] `elapsed()` calculation is accurate
- [ ] `is_warning()` returns true only after threshold
- [ ] Duration formatting (seconds, minutes, hours)

## Success Criteria

1. Users can see at a glance how long the current task has been active
2. Subtle warning draws attention when task has been active >15 min
3. No backend changes required
4. Minimal performance impact (1 render per second is acceptable)

## Future Enhancements

- Configurable warning threshold via GUI settings
- Persist cumulative time on task entities (requires backend changes)
- Multiple warning levels (15m yellow, 30m orange, 1h red)
- Audio/notification alerts (opt-in)
- Time tracking reports
