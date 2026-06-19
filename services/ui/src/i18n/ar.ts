import { mergeDeep } from './mergeDeep';
import { en, type Messages } from './en';
import { arOverlay } from './arOverlay';

export const ar = mergeDeep(en as Record<string, unknown>, arOverlay) as Messages;
