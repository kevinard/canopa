# Safe configuration candidates

Notification Configuration remains KDL under ADR 0010, but the supported application schema is explicit: only rule, sound and icons nodes, no child documents or type annotations, and only the documented entries. Reject unsupported shapes rather than silently applying a subset. Failed candidates preserve the committed path, generation, rules and presentation settings under ADR 0027.

Read UTF-8 configuration only from an opened regular-file descriptor, allowing symlinks to regular files, with a 1 MiB source limit and one overflow probe. Use the existing descriptor-safe reader so FIFOs/devices cannot become configuration streams. Prepare candidates outside asynchronous executor threads; reload serialization remains owned by the actual preparation job through installation, including caller cancellation. This bounds source bytes and concurrent reload preparation, not parser-internal allocation, nesting/work, filesystem latency or total process memory.

Configuration errors describe the unsupported setting category or syntax location without copying configuration values, arbitrary node/property names or parser source text. The selected committed file path remains available explicitly in status. No configuration format version, installed default, freedesktop capability or protocol change follows from this validation policy.
