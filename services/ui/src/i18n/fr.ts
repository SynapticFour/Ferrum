import { mergeDeep } from './mergeDeep';
import { en, type Messages } from './en';
import { frOverlay } from './frOverlay';

export const fr = mergeDeep(en as Record<string, unknown>, frOverlay) as Messages;
