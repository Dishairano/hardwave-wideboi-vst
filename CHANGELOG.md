# Hardwave WideBoi — Changelog

## v0.4.2

Three faults found by the automated testers, not reported by anyone.

- **A damaged project file could take the whole DAW down.** Loading a corrupt
  or foreign state made the plug-in ask for an impossible amount of memory, and
  the failed request killed the host process rather than the plug-in. It now
  refuses the state and carries on.
- **Your settings came back, but the DAW did not know.** Reopening a project
  restored every control inside WideBoi, and the plug-in never told the host to
  re-read them. A host that trusts its own copy showed and automated the old
  values, so a project could sound different from what the controls said.
- **Typed values were ignored on most controls.** Only a control whose text
  happened to match its own unit accepted a typed number, so anything printing
  its own format refused what you typed and snapped back. Every control takes a
  typed number now, and a displayed value survives being typed back in.

Also: the macOS build is genuinely universal. It was labelled universal while
holding an Apple Silicon binary only, which an Intel Mac reports as "failed to
scan" and nothing else.

## v0.4.0 — Multiband stereo widener (2026-05-24)

- Three independent width bands (low / mid / high) instead of one global knob — mono your kick and sub to keep the low end tight while you push the mids and highs wide. The thing every stereo tool should let you do.
- Live goniometer (vectorscope) so you can SEE your stereo field — a vertical line means mono, a horizontal spread means wide.
- Per-band correlation read-outs that warn you (red) when a band goes mono-incompatible, so you don't ship something that collapses on a club PA or phone speaker.
- Two crossover controls to place where the low/mid and mid/high splits happen.
- Phase-coherent Linkwitz-Riley crossovers — the bands recombine flat with no notch at the crossover points.
