# empty-search.png

- **Date:** 2026-09-12
- **Model:** `openai/gpt-image-2.5-sunburst`
- **Size:** 512×512 (generated at 1024², downscaled)
- **Use:** art-glyph for the "Search your library" and "No matches" empty states in `SearchView`.
- **Prompt:** Minimal line-art illustration centred on a perfectly flat solid charcoal-black background (#0B0B0E), square 1:1, absolutely no text, no letters, no numbers, no logos, no watermark, no UI. A single magnifying glass drawn in thin rounded royal-blue (#2B5CE6) outline strokes, tilted, with three small concentric sound-wave arcs inside its lens instead of a reflection. Restrained and matte, not neon, no bloom, no glow halo. Sparse and calm with generous empty black space around the subject, flat vector look, even stroke weight, no fill, no shading. Strict palette: charcoal black background, royal blue line work only.
- **Post-processing:** the model renders line art on a solid charcoal background, which shows as a square block over the app's ambient wash. The backdrop is keyed to alpha by distance from the corner colour, so anti-aliased stroke edges fade rather than stair-step.
