# Ideas

- [x] Add tag filtering to web view
- [ ] Add meta nodes in web view of tags
- [x] Make graph view more circular without a harsh boundary so the graph doesnt squish
- [x] Support zooming in web view
- [x] Add a --desc shorthand for --description flag, make similar 4 letter abbreviations for other long flags.
- [x] Add an information pannel showing the full description for the selected node in the graph view
- [ ] Let the graph handle inserted/deleted nodes without jostling the whole thing
- [x] Add arrows to the springs so its clear which direction the dependency is, the pointy bit of the arrow should be on the dependant task to present a logical flow of following the arrows until completion
- [ ] Enable theming support as a configuration option for `bn gui`
- [x] Make tests use a separate bn storage that isn't the actual repo's storage to avoid polluting the real task graph with test fixtures (BN_DATA_DIR env var)
- [ ] Offer to include auto-approve rules for copilot, codex, opencode, and claude
- [x] Add the concept of a question that can be linked to any other resource type, and a user then links an answer off of it → See `prds/PRD_QUESTION_ANSWER_NODES.md`
- [x] Make `bn gui` automatically pick the first available port
- [ ] Require bugs to be linked to existing tasks (or other objects) so we can trace bugs back to the original intent of the feature
- [x] Add PRD as a first-class object in binnacle, programmatically linked to tasks so each task can trace back to its owning PRD (instead of separate .md files) → See `prds/PRD_PRD_NODES.md`
- [ ] using git notes to send small sub-graphs of bn to agents so they can just focus on their problem domain and not get sidetracked, then when they PR it back they send back their updated graph and we merge it into the main graph, which shouldnt have conflicts because we know what nodes its touched and have locked them, plus we can canonicalize any bn-1234
   notation they have to avoid conflicts on the merge
- [x] Add a visual highlight for end goal tasks in the web UI (tasks with no dependants)
