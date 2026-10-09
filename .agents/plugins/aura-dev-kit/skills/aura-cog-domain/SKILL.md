---
name: aura-cog-domain
description: Cognitive Layer constraints. Activate this skill when working on the
  cog domain.
---
# COG Domain Rules
- **API**: Use Gemini Interactions API exclusively.
- **Turns**: Pass `previous_interaction_id` instead of resending history.
- **Streams**: Consume SSE streams, expose structured events.
- **Function Calling**: Intercept, run in sandboxes, inject results back.
- **Background**: Long-horizon tasks use `background=true`.
- **Retrieval**: Use `file_search_stores`. Explicitly delete them on task completion.
- **Security**: API credentials never enter model context or agent memory.
