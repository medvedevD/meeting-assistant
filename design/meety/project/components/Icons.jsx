// Icons + small primitives shared by Meety screens. Stroke icons only — quiet, editorial.

function Icon({ name, size = 16, className = '' }) {
  const props = { width: size, height: size, viewBox: '0 0 24 24', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round', className: `icn ${className}`, style: { width: size, height: size } };
  switch (name) {
    case 'mic': return <svg {...props}><rect x="9" y="3" width="6" height="12" rx="3"/><path d="M5 11a7 7 0 0 0 14 0"/><path d="M12 18v3"/></svg>;
    case 'play': return <svg {...props}><path d="M7 5l12 7-12 7z" fill="currentColor"/></svg>;
    case 'pause': return <svg {...props}><rect x="7" y="5" width="3.5" height="14" rx="1" fill="currentColor"/><rect x="13.5" y="5" width="3.5" height="14" rx="1" fill="currentColor"/></svg>;
    case 'stop': return <svg {...props}><rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor"/></svg>;
    case 'plus': return <svg {...props}><path d="M12 5v14M5 12h14"/></svg>;
    case 'refresh': return <svg {...props}><path d="M3 12a9 9 0 0 1 15.5-6.2L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 0 1-15.5 6.2L3 16"/><path d="M3 21v-5h5"/></svg>;
    case 'search': return <svg {...props}><circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/></svg>;
    case 'gear': return <svg {...props}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.9 2.9l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.9-2.9l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.9-2.9l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.9 2.9l-.1.1a1.7 1.7 0 0 0-.3 1.8V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/></svg>;
    case 'sparkle': return <svg {...props}><path d="M12 3v3M12 18v3M3 12h3M18 12h3M5.6 5.6l2.1 2.1M16.3 16.3l2.1 2.1M5.6 18.4l2.1-2.1M16.3 7.7l2.1-2.1"/></svg>;
    case 'doc': return <svg {...props}><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><path d="M14 3v6h6"/><path d="M9 14h6M9 17h6M9 11h2"/></svg>;
    case 'folder': return <svg {...props}><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>;
    case 'arrow-left': return <svg {...props}><path d="M19 12H5M12 5l-7 7 7 7"/></svg>;
    case 'check': return <svg {...props}><path d="m5 13 4 4L19 7"/></svg>;
    case 'more': return <svg {...props}><circle cx="5" cy="12" r="1.5" fill="currentColor"/><circle cx="12" cy="12" r="1.5" fill="currentColor"/><circle cx="19" cy="12" r="1.5" fill="currentColor"/></svg>;
    case 'waveform': return <svg {...props}><path d="M4 10v4M8 7v10M12 4v16M16 7v10M20 10v4"/></svg>;
    case 'download': return <svg {...props}><path d="M12 3v12M7 10l5 5 5-5M5 21h14"/></svg>;
    case 'copy': return <svg {...props}><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/></svg>;
    case 'share': return <svg {...props}><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="m8.2 11 7.6-4M8.2 13l7.6 4"/></svg>;
    case 'eye': return <svg {...props}><path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/></svg>;
    case 'trash': return <svg {...props}><path d="M3 6h18M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2M6 6l1 14a2 2 0 0 0 2 2h6a2 2 0 0 0 2-2l1-14"/></svg>;
    case 'speakers': return <svg {...props}><circle cx="9" cy="9" r="3"/><path d="M3 19a6 6 0 0 1 12 0"/><circle cx="17" cy="7" r="2.5"/><path d="M14 16a5 5 0 0 1 7 0"/></svg>;
    case 'edit': return <svg {...props}><path d="M11 4H4v16h16v-7"/><path d="M18 2 22 6l-12 12H6v-4z"/></svg>;
    case 'split': return <svg {...props}><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M12 4v16"/></svg>;
    case 'list': return <svg {...props}><path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/></svg>;
    case 'card': return <svg {...props}><rect x="3" y="4" width="18" height="6" rx="1.5"/><rect x="3" y="14" width="18" height="6" rx="1.5"/></svg>;
    case 'book': return <svg {...props}><path d="M4 4a2 2 0 0 1 2-2h14v18H6a2 2 0 0 0-2 2z"/><path d="M4 4v18"/></svg>;
    case 'clock': return <svg {...props}><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>;
    case 'user': return <svg {...props}><circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/></svg>;
    case 'team': return <svg {...props}><circle cx="9" cy="8" r="3.5"/><path d="M2.5 20a6.5 6.5 0 0 1 13 0"/><circle cx="17" cy="6" r="2.5"/><path d="M14.5 13.5A5 5 0 0 1 21.5 18"/></svg>;
    case 'cpu': return <svg {...props}><rect x="6" y="6" width="12" height="12" rx="1.5"/><rect x="9" y="9" width="6" height="6"/><path d="M10 2v3M14 2v3M10 19v3M14 19v3M2 10h3M2 14h3M19 10h3M19 14h3"/></svg>;
    case 'storage': return <svg {...props}><ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14a8 3 0 0 0 16 0V5"/><path d="M4 12a8 3 0 0 0 16 0"/></svg>;
    case 'key': return <svg {...props}><circle cx="8" cy="15" r="4"/><path d="m10.8 12.2 9.2-9.2M16 5l3 3M14 7l3 3"/></svg>;
    case 'sun': return <svg {...props}><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/></svg>;
    case 'moon': return <svg {...props}><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/></svg>;
    default: return null;
  }
}

// Typographic mark — a confident italic serif lowercase "m" with a record-dot
// substituting the tittle of the implied "i". The dot is the accent color and
// telegraphs both the brand and the product (it records).
function Wordmark({ size = 28, accent }) {
  const dot = accent || 'var(--accent)';
  const fontSize = Math.round(size * 1.05);
  const dotR = Math.max(2.4, size * 0.11);
  return (
    <div
      aria-hidden="true"
      style={{
        height: size,
        position: 'relative',
        display: 'inline-flex',
        alignItems: 'center',
        paddingRight: dotR * 2 + 2,
      }}
    >
      <span style={{
        fontFamily: 'var(--font-serif)',
        fontStyle: 'italic',
        fontWeight: 500,
        fontSize: fontSize,
        lineHeight: 1,
        color: 'var(--ink)',
        letterSpacing: '-0.03em',
        display: 'inline-block',
      }}>m</span>
      <span style={{
        position: 'absolute',
        top: Math.round(size * 0.06),
        right: 0,
        width: dotR * 2, height: dotR * 2,
        background: dot,
        borderRadius: '50%',
      }} />
    </div>
  );
}

// Hero size — bigger, no tile background.
function WordmarkHero({ size = 96, accent }) {
  const dot = accent || 'var(--accent)';
  const dotR = size * 0.075;
  return (
    <div style={{ position: 'relative', display: 'inline-block', width: size, height: size * 0.95 }} aria-hidden="true">
      <span style={{
        fontFamily: 'var(--font-serif)',
        fontStyle: 'italic',
        fontWeight: 500,
        fontSize: size * 1.05,
        lineHeight: 1,
        color: 'var(--ink)',
        letterSpacing: '-0.03em',
      }}>m</span>
      <span style={{
        position: 'absolute',
        top: size * 0.05,
        right: -size * 0.02,
        width: dotR * 2, height: dotR * 2,
        background: dot,
        borderRadius: '50%',
      }} />
    </div>
  );
}

// Status badge — small dot + label
function StatusDot({ kind = 'idle' }) {
  const map = {
    idle: { color: 'var(--ink-4)', label: '' },
    rec: { color: 'var(--rec)', label: 'rec' },
    transcribed: { color: 'var(--accent)', label: '' },
    done: { color: 'var(--ok)', label: '' },
  };
  return <span className="dot" style={{ background: map[kind]?.color || map.idle.color }} />;
}

Object.assign(window, { Icon, Wordmark, WordmarkHero, StatusDot });
