export interface Theme {
  id: string;
  name: string;
  vars: Record<string, string>;
}

export const THEMES: Theme[] = [
  {
    id: 'obsidian',
    name: 'Obsidian',
    vars: {
      '--bg': '#0a0a0a', '--surface': '#0f0f0f', '--card': '#141414', '--input': '#1e1e1e',
      '--line': '#282828', '--line-hi': '#383838',
      '--fg': '#f1f1f1', '--fg-2': '#cccccc', '--fg-3': '#888888',
      '--fg-4': '#555555', '--fg-5': '#444444', '--fg-6': '#333333',
      '--cta': '#ffffff', '--cta-hv': '#e0e0e0', '--cta-fg': '#000000',
      '--tint': '#1e1e1e', '--tint-bd': '#2a2a2a',
    },
  },
  {
    id: 'paper',
    name: 'Paper',
    vars: {
      '--bg': '#fafafa', '--surface': '#f3f3f3', '--card': '#ebebeb', '--input': '#e0e0e0',
      '--line': '#d0d0d0', '--line-hi': '#b8b8b8',
      '--fg': '#111111', '--fg-2': '#2e2e2e', '--fg-3': '#555555',
      '--fg-4': '#808080', '--fg-5': '#999999', '--fg-6': '#c0c0c0',
      '--cta': '#111111', '--cta-hv': '#333333', '--cta-fg': '#ffffff',
      '--tint': '#e2e2e2', '--tint-bd': '#cacaca',
    },
  },
  {
    id: 'github-light',
    name: 'GitHub Light',
    vars: {
      '--bg': '#ffffff', '--surface': '#f6f8fa', '--card': '#eaeef2', '--input': '#d0d7de',
      '--line': '#d0d7de', '--line-hi': '#afb8c1',
      '--fg': '#1f2328', '--fg-2': '#24292f', '--fg-3': '#57606a',
      '--fg-4': '#6e7781', '--fg-5': '#8c959f', '--fg-6': '#c9d1d9',
      '--cta': '#1f2328', '--cta-hv': '#333a42', '--cta-fg': '#ffffff',
      '--tint': '#eaecf0', '--tint-bd': '#dce0e6',
    },
  },
  {
    id: 'solarized-light',
    name: 'Solarized Light',
    vars: {
      '--bg': '#fdf6e3', '--surface': '#eee8d5', '--card': '#e8e2cf', '--input': '#ddd6c1',
      '--line': '#cdc7b3', '--line-hi': '#b8b2a0',
      '--fg': '#073642', '--fg-2': '#002b36', '--fg-3': '#586e75',
      '--fg-4': '#657b83', '--fg-5': '#839496', '--fg-6': '#93a1a1',
      '--cta': '#268bd2', '--cta-hv': '#3da0e5', '--cta-fg': '#fdf6e3',
      '--tint': '#e8e4d8', '--tint-bd': '#dad6c8',
    },
  },
  {
    id: 'catppuccin-latte',
    name: 'Catppuccin Latte',
    vars: {
      '--bg': '#eff1f5', '--surface': '#e6e9ef', '--card': '#dce0e8', '--input': '#ccd0da',
      '--line': '#bcc0cc', '--line-hi': '#acb0be',
      '--fg': '#4c4f69', '--fg-2': '#5c5f77', '--fg-3': '#6c6f85',
      '--fg-4': '#6b6e82', '--fg-5': '#7c7f93', '--fg-6': '#9ca0b0',
      '--cta': '#8839ef', '--cta-hv': '#9a4ff7', '--cta-fg': '#eff1f5',
      '--tint': '#e2e5ec', '--tint-bd': '#d4d8e2',
    },
  },
  {
    id: 'ayu-light',
    name: 'Ayu Light',
    vars: {
      '--bg': '#f8f9fa', '--surface': '#f0f1f2', '--card': '#e8eaec', '--input': '#dde0e4',
      '--line': '#d0d4d8', '--line-hi': '#b8bdc4',
      '--fg': '#232834', '--fg-2': '#3d4350', '--fg-3': '#5c6570',
      '--fg-4': '#8a9199', '--fg-5': '#a8b0b8', '--fg-6': '#c8d0d8',
      '--cta': '#ff9940', '--cta-hv': '#ffae65', '--cta-fg': '#f8f9fa',
      '--tint': '#eceef0', '--tint-bd': '#e0e4e8',
    },
  },
  {
    id: 'tokyo-night',
    name: 'Tokyo Night',
    vars: {
      '--bg': '#1a1b26', '--surface': '#16161e', '--card': '#1f2335', '--input': '#24283b',
      '--line': '#292e42', '--line-hi': '#3b4261',
      '--fg': '#c0caf5', '--fg-2': '#a9b1d6', '--fg-3': '#9aa5ce',
      '--fg-4': '#565f89', '--fg-5': '#414868', '--fg-6': '#2f334d',
      '--cta': '#7aa2f7', '--cta-hv': '#89b4fa', '--cta-fg': '#1a1b26',
      '--tint': '#1e2438', '--tint-bd': '#2a3355',
    },
  },
  {
    id: 'tokyo-storm',
    name: 'Tokyo Night Storm',
    vars: {
      '--bg': '#24283b', '--surface': '#1f2335', '--card': '#292e42', '--input': '#2f3549',
      '--line': '#3b4261', '--line-hi': '#545c7e',
      '--fg': '#c0caf5', '--fg-2': '#a9b1d6', '--fg-3': '#9aa5ce',
      '--fg-4': '#565f89', '--fg-5': '#414868', '--fg-6': '#2f334d',
      '--cta': '#7aa2f7', '--cta-hv': '#89b4fa', '--cta-fg': '#1a1b26',
      '--tint': '#2a3050', '--tint-bd': '#353d65',
    },
  },
  {
    id: 'dracula',
    name: 'Dracula',
    vars: {
      '--bg': '#282a36', '--surface': '#21222c', '--card': '#2d2f3f', '--input': '#383a4b',
      '--line': '#44475a', '--line-hi': '#6272a4',
      '--fg': '#f8f8f2', '--fg-2': '#e0dff5', '--fg-3': '#a0a0c0',
      '--fg-4': '#6272a4', '--fg-5': '#4f5570', '--fg-6': '#383a4b',
      '--cta': '#bd93f9', '--cta-hv': '#caa9fa', '--cta-fg': '#282a36',
      '--tint': '#312c48', '--tint-bd': '#423862',
    },
  },
  {
    id: 'one-dark',
    name: 'One Dark Pro',
    vars: {
      '--bg': '#282c34', '--surface': '#21252b', '--card': '#2c313a', '--input': '#3b4048',
      '--line': '#3b4048', '--line-hi': '#4b5263',
      '--fg': '#d7dae0', '--fg-2': '#abb2bf', '--fg-3': '#9da5b4',
      '--fg-4': '#636d83', '--fg-5': '#4b5263', '--fg-6': '#383e4a',
      '--cta': '#61afef', '--cta-hv': '#7bbfff', '--cta-fg': '#282c34',
      '--tint': '#2a3245', '--tint-bd': '#344055',
    },
  },
  {
    id: 'catppuccin',
    name: 'Catppuccin Mocha',
    vars: {
      '--bg': '#1e1e2e', '--surface': '#181825', '--card': '#313244', '--input': '#45475a',
      '--line': '#45475a', '--line-hi': '#585b70',
      '--fg': '#cdd6f4', '--fg-2': '#bac2de', '--fg-3': '#a6adc8',
      '--fg-4': '#7f849c', '--fg-5': '#6c7086', '--fg-6': '#585b70',
      '--cta': '#cba6f7', '--cta-hv': '#d8b4fe', '--cta-fg': '#1e1e2e',
      '--tint': '#2a2640', '--tint-bd': '#3a3660',
    },
  },
  {
    id: 'nord',
    name: 'Nord',
    vars: {
      '--bg': '#2e3440', '--surface': '#242933', '--card': '#3b4252', '--input': '#434c5e',
      '--line': '#4c566a', '--line-hi': '#5e6779',
      '--fg': '#eceff4', '--fg-2': '#e5e9f0', '--fg-3': '#d8dee9',
      '--fg-4': '#81909d', '--fg-5': '#677282', '--fg-6': '#4c566a',
      '--cta': '#88c0d0', '--cta-hv': '#9ecfde', '--cta-fg': '#2e3440',
      '--tint': '#334055', '--tint-bd': '#3e4e68',
    },
  },
  {
    id: 'gruvbox',
    name: 'Gruvbox Dark',
    vars: {
      '--bg': '#282828', '--surface': '#1d2021', '--card': '#3c3836', '--input': '#504945',
      '--line': '#504945', '--line-hi': '#665c54',
      '--fg': '#ebdbb2', '--fg-2': '#d5c4a1', '--fg-3': '#bdae93',
      '--fg-4': '#928374', '--fg-5': '#7c6f64', '--fg-6': '#665c54',
      '--cta': '#fabd2f', '--cta-hv': '#fcd56e', '--cta-fg': '#282828',
      '--tint': '#38321e', '--tint-bd': '#4a4222',
    },
  },
  {
    id: 'monokai',
    name: 'Monokai',
    vars: {
      '--bg': '#272822', '--surface': '#1e1f1c', '--card': '#2d2e27', '--input': '#3e3d32',
      '--line': '#49483e', '--line-hi': '#75715e',
      '--fg': '#f8f8f2', '--fg-2': '#e0e0d0', '--fg-3': '#cfcfba',
      '--fg-4': '#75715e', '--fg-5': '#49483e', '--fg-6': '#3e3d32',
      '--cta': '#a6e22e', '--cta-hv': '#c4f250', '--cta-fg': '#272822',
      '--tint': '#303425', '--tint-bd': '#404830',
    },
  },
  {
    id: 'solarized',
    name: 'Solarized Dark',
    vars: {
      '--bg': '#002b36', '--surface': '#00212b', '--card': '#073642', '--input': '#0d3d4a',
      '--line': '#1a4a58', '--line-hi': '#586e75',
      '--fg': '#eee8d5', '--fg-2': '#d9d0c0', '--fg-3': '#93a1a1',
      '--fg-4': '#839496', '--fg-5': '#657b83', '--fg-6': '#586e75',
      '--cta': '#268bd2', '--cta-hv': '#3da0e5', '--cta-fg': '#002b36',
      '--tint': '#0a3848', '--tint-bd': '#1a5060',
    },
  },
  {
    id: 'ayu',
    name: 'Ayu Mirage',
    vars: {
      '--bg': '#1f2430', '--surface': '#191e2a', '--card': '#242b38', '--input': '#2d3446',
      '--line': '#343d50', '--line-hi': '#434f63',
      '--fg': '#cbccc6', '--fg-2': '#b0b8c6', '--fg-3': '#8695a8',
      '--fg-4': '#5c6773', '--fg-5': '#4a535e', '--fg-6': '#343d50',
      '--cta': '#ffcc66', '--cta-hv': '#ffd988', '--cta-fg': '#1f2430',
      '--tint': '#2e3328', '--tint-bd': '#3e4335',
    },
  },
  {
    id: 'cafe',
    name: 'Café',
    vars: {
      '--bg': '#1c1410', '--surface': '#231a13', '--card': '#2d2018', '--input': '#3d2e22',
      '--line': '#4d3e30', '--line-hi': '#6b5540',
      '--fg': '#e8d5b7', '--fg-2': '#d4b896', '--fg-3': '#b89870',
      '--fg-4': '#8b7355', '--fg-5': '#6b5540', '--fg-6': '#4d3e30',
      '--cta': '#c8843a', '--cta-hv': '#da9a50', '--cta-fg': '#1c1410',
      '--tint': '#302218', '--tint-bd': '#423020',
    },
  },
  {
    id: 'rose-pine',
    name: 'Rosé Pine',
    vars: {
      '--bg': '#191724', '--surface': '#1f1d2e', '--card': '#26233a', '--input': '#2e2a3f',
      '--line': '#403d52', '--line-hi': '#524f67',
      '--fg': '#e0def4', '--fg-2': '#d4d0ee', '--fg-3': '#908caa',
      '--fg-4': '#6e6a86', '--fg-5': '#524f67', '--fg-6': '#403d52',
      '--cta': '#ebbcba', '--cta-hv': '#f0cac8', '--cta-fg': '#191724',
      '--tint': '#252038', '--tint-bd': '#332c4e',
    },
  },
  {
    id: 'everforest',
    name: 'Everforest',
    vars: {
      '--bg': '#272e33', '--surface': '#2e383c', '--card': '#374145', '--input': '#414b50',
      '--line': '#4f5b58', '--line-hi': '#5f6d69',
      '--fg': '#d3c6aa', '--fg-2': '#c5b99a', '--fg-3': '#9da9a0',
      '--fg-4': '#7a8478', '--fg-5': '#5c6a6a', '--fg-6': '#4f5b58',
      '--cta': '#a7c080', '--cta-hv': '#b8d090', '--cta-fg': '#272e33',
      '--tint': '#304038', '--tint-bd': '#3e5048',
    },
  },
  {
    id: 'kanagawa',
    name: 'Kanagawa',
    vars: {
      '--bg': '#1f1f28', '--surface': '#16161d', '--card': '#2a2a37', '--input': '#363646',
      '--line': '#363646', '--line-hi': '#54546d',
      '--fg': '#dcd7ba', '--fg-2': '#c8c093', '--fg-3': '#a9a6c1',
      '--fg-4': '#7a7880', '--fg-5': '#54546d', '--fg-6': '#363646',
      '--cta': '#7e9cd8', '--cta-hv': '#9fb4e8', '--cta-fg': '#1f1f28',
      '--tint': '#252538', '--tint-bd': '#303050',
    },
  },
  {
    id: 'material',
    name: 'Material Dark',
    vars: {
      '--bg': '#212121', '--surface': '#1a1a1a', '--card': '#2b2b2b', '--input': '#323232',
      '--line': '#3a3a3a', '--line-hi': '#4a4a4a',
      '--fg': '#f0f0f0', '--fg-2': '#d0d0d0', '--fg-3': '#a0a0a0',
      '--fg-4': '#707070', '--fg-5': '#555555', '--fg-6': '#3a3a3a',
      '--cta': '#c8c8c8', '--cta-hv': '#e0e0e0', '--cta-fg': '#212121',
      '--tint': '#303030', '--tint-bd': '#3e3e3e',
    },
  },
];

export function applyTheme(theme: Theme) {
  const root = document.documentElement;
  for (const [key, val] of Object.entries(theme.vars)) {
    root.style.setProperty(key, val);
  }
}

export function getTheme(id: string): Theme {
  return THEMES.find(t => t.id === id) ?? THEMES[0];
}
