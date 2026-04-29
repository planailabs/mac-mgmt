/** @type {import('tailwindcss').Config} */
//
// Color tokens are defined as CSS variables in input.css. Each Tailwind
// color resolves to `rgb(var(--c-x) / <alpha-value>)` so opacity modifiers
// (e.g. `bg-brand/40`) keep working. Light/dark themes are pure CSS-variable
// swaps — utility classes like `bg-brand` automatically follow the theme.
//
// Adding a new design token: declare the CSS variable in input.css under
// both `:root` and `.dark`, then add a key here.
const tokenColor = (v) => `rgb(var(--${v}) / <alpha-value>)`;

module.exports = {
  darkMode: 'selector',
  content: ["./src/**/*.rs"],
  // `td` and `th` collide with HTML element names; Tailwind's content
  // extractor heuristically drops them. Safelist so the @layer rules
  // for our `<Td>` / `<TdMono>` / `<TdMuted>` cells survive purge.
  safelist: ['td', 'th'],
  theme: {
    extend: {
      colors: {
        brand: {
          DEFAULT: tokenColor('c-brand'),
          strong:  tokenColor('c-brand-strong'),
          soft:    tokenColor('c-brand-soft'),
        },
        surface: {
          DEFAULT: tokenColor('c-surface'),
          '2':     tokenColor('c-surface-2'),
          '3':     tokenColor('c-surface-3'),
        },
        fg: {
          DEFAULT: tokenColor('c-fg'),
          strong:  tokenColor('c-fg-strong'),
          muted:   tokenColor('c-fg-muted'),
          faint:   tokenColor('c-fg-faint'),
          invert:  tokenColor('c-fg-invert'),
        },
        line: {
          DEFAULT: tokenColor('c-line'),
          soft:    tokenColor('c-line-soft'),
        },
        danger: {
          DEFAULT: tokenColor('c-danger'),
          strong:  tokenColor('c-danger-strong'),
          soft:    tokenColor('c-danger-soft'),
        },
        warn: {
          DEFAULT: tokenColor('c-warn'),
          strong:  tokenColor('c-warn-strong'),
          soft:    tokenColor('c-warn-soft'),
        },
        success: {
          DEFAULT: tokenColor('c-success'),
          soft:    tokenColor('c-success-soft'),
        },
        info: {
          DEFAULT: tokenColor('c-info'),
          soft:    tokenColor('c-info-soft'),
        },
        accent: {
          DEFAULT: tokenColor('c-accent'),
          strong:  tokenColor('c-accent-strong'),
          soft:    tokenColor('c-accent-soft'),
        },
      },
      boxShadow: {
        card: 'var(--shadow-card)',
      },
    },
  },
  plugins: [require("@tailwindcss/typography")],
};
