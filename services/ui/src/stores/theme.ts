import { create } from 'zustand';
import { persist } from 'zustand/middleware';

type ThemeStore = { dark: boolean; toggle: () => void };

export const useThemeStore = create<ThemeStore>()(
  persist(
    (set) => ({ dark: true, toggle: () => set((s) => ({ dark: !s.dark })) }),
    { name: 'ferrum-theme' }
  )
);
