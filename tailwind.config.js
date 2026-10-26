/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        base: "var(--bg-base)",
        surface: "var(--bg-surface)",
        elevated: "var(--bg-elevated)",
        secondary: "var(--bg-surface)",
        tertiary: "var(--bg-elevated)",
        hover: "var(--bg-hover)",
        active: "var(--bg-active)",
        
        "border-subtle": "var(--border-subtle)",
        
        "accent-primary": "var(--accent-primary)",
        "accent-secondary": "var(--accent-secondary)",
        "accent-tertiary": "var(--accent-tertiary)",
        "accent-glow": "var(--accent-glow)",
        
        "text-primary": "var(--text-primary)",
        "text-secondary": "var(--text-secondary)",
        "text-muted": "var(--text-muted)",
        primary: "var(--text-primary)",
        secondary_text: "var(--text-secondary)",
        muted: "var(--text-muted)",
        
        success: "var(--success)",
        danger: "var(--danger)",
        warning: "var(--warning)",
        
        status: {
          online: "var(--status-online)",
          idle: "var(--status-idle)",
          dnd: "var(--status-dnd)",
          offline: "var(--status-offline)",
        }
      },
      borderRadius: {
        DEFAULT: "var(--radius-default)",
        lg: "var(--radius-lg)",
        xl: "var(--radius-xl)",
      },
      boxShadow: {
        'glow-sm': '0 0 12px var(--accent-glow)',
        'glow-md': '0 0 20px var(--accent-glow)',
        'glow-active': '0 0 12px var(--accent-glow)',
      },
      transitionDuration: {
        DEFAULT: '180ms',
        gamer: '200ms',
      }
    },
  },
  plugins: [],
}
