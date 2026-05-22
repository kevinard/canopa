# Rich Dmenu Input Uses JSON Lines

Rich dmenu input uses JSON Lines on standard input: one structured entry object per line. This preserves dmenu's candidate-per-record model and composes with shell pipelines while allowing entries to provide separate labels, returned values, icons, and search keywords.

We considered a single JSON array payload, extending delimiter-based fields, and using Niribar's KDL configuration shape. A JSON array would make whole-payload validation natural but would be less stream- and pipeline-friendly; delimiter extensions would be fragile once optional metadata is added; KDL would blur request-owned dmenu input with persistent Niribar configuration.
