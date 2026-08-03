/**
 * Renders an FTS5 `snippet()` result that contains `<mark>`/`</mark>` markup
 * around matched terms, without `dangerouslySetInnerHTML`.
 *
 * The snippet text comes from our own SQLite FTS5 index over meeting
 * transcripts, not third-party input — but a meeting participant could still
 * say something containing literal `<`/`>` characters that ends up in the
 * transcript and therefore in the snippet. Splitting on the two literal
 * `<mark>` / `</mark>` tags and rendering everything else as plain text (via
 * React's normal text-node escaping) highlights matches without ever parsing
 * arbitrary HTML.
 */

const MARK_OPEN = '<mark>';
const MARK_CLOSE = '</mark>';

export default function HighlightedSnippet({ snippet }: { snippet: string }) {
  const nodes: React.ReactNode[] = [];
  let rest = snippet;
  let key = 0;

  while (rest.length > 0) {
    const openIdx = rest.indexOf(MARK_OPEN);
    if (openIdx === -1) {
      nodes.push(<span key={key++}>{rest}</span>);
      break;
    }

    if (openIdx > 0) {
      nodes.push(<span key={key++}>{rest.slice(0, openIdx)}</span>);
    }

    const afterOpen = rest.slice(openIdx + MARK_OPEN.length);
    const closeIdx = afterOpen.indexOf(MARK_CLOSE);
    if (closeIdx === -1) {
      // Unbalanced markup (shouldn't happen from our own snippet() output,
      // but never trust it blindly) - render the rest as plain text instead
      // of dropping it or guessing where the highlight should end.
      nodes.push(<span key={key++}>{afterOpen}</span>);
      break;
    }

    nodes.push(
      <mark key={key++} className="rounded bg-amber-200 px-0.5 text-stone-900">
        {afterOpen.slice(0, closeIdx)}
      </mark>
    );
    rest = afterOpen.slice(closeIdx + MARK_CLOSE.length);
  }

  return <>{nodes}</>;
}
