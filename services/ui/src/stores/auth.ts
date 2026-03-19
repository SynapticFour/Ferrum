import { create } from 'zustand';

type AuthStore = { passportJwt: string | null; setPassport: (jwt: string | null) => void };

export const useAuthStore = create<AuthStore>((set) => ({
  passportJwt: null,
  setPassport: (jwt) => set({ passportJwt: jwt }),
}));
