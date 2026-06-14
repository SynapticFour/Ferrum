import { create } from 'zustand';
import { loadStoredPassport, storePassport } from '@/lib/auth';

type AuthStore = { passportJwt: string | null; setPassport: (jwt: string | null) => void };

export const useAuthStore = create<AuthStore>((set) => ({
  passportJwt: loadStoredPassport(),
  setPassport: (jwt) => {
    storePassport(jwt);
    set({ passportJwt: jwt });
  },
}));
